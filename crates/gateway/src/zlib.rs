//! zlib-stream inflate for one gateway connection.

use flate2::{Decompress, FlushDecompress, Status};

use crate::error::GatewayError;

/// Suffix Discord writes after each zlib-stream payload (`Z_SYNC_FLUSH`).
pub(crate) const ZLIB_SUFFIX: [u8; 4] = [0, 0, 0xFF, 0xFF];

const SHRINK_AFTER: usize = 1024 * 1024;
const SHRINK_TO: usize = 8 * 1024;

/// One inflate context for the lifetime of a socket.
pub struct ZlibStream {
    inflate: Decompress,
    pending: Vec<u8>,
    output: Vec<u8>,
    max_message: usize,
}

impl ZlibStream {
    /// New context with the default cap (tests; sessions pass their cap).
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_limit(crate::types::DEFAULT_MAX_MESSAGE_BYTES)
    }

    /// New context; every connection (including a resume) starts one.
    pub fn with_limit(max_message: usize) -> Self {
        Self {
            inflate: Decompress::new(true),
            pending: Vec::with_capacity(4096),
            output: Vec::with_capacity(4096),
            max_message,
        }
    }

    /// Feed a WebSocket binary frame. Completed JSON documents are returned
    /// in order (a frame may contain several, or only part of one).
    pub fn push(&mut self, data: &[u8]) -> Result<Vec<Box<str>>, GatewayError> {
        let mut out = Vec::new();
        let mut buf = Vec::with_capacity(4096);
        self.push_each(data, &mut buf, |json| out.push(Box::from(json)))?;
        Ok(out)
    }

    /// Fast-path single-message inflate: when `pending` is empty and `data`
    /// is exactly one complete zlib-stream frame, inflates directly from
    /// `data` into `out` with zero intermediate copies or heap allocations.
    ///
    /// Returns `Ok(true)` if `out` holds the single completed UTF-8 JSON
    /// document, or `Ok(false)` if the caller should fall back to
    /// [`Self::push`] (split frame or multiple messages in one frame).
    pub(crate) fn try_inflate_single(
        &mut self,
        data: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<bool, GatewayError> {
        if !self.pending.is_empty() || data.len() < 4 || !data.ends_with(&ZLIB_SUFFIX) {
            return Ok(false);
        }
        // Only take the fast path when there is no earlier ZLIB_SUFFIX inside `data`.
        if find_suffix(&data[..data.len() - 1]).is_some() {
            return Ok(false);
        }
        if data.len() > self.max_message {
            return Err(GatewayError::zlib("compressed message exceeds size cap"));
        }
        self.inflate_into(data, out)?;
        let _ = std::str::from_utf8(out)
            .map_err(|_| GatewayError::zlib("decompressed payload is not UTF-8"))?;
        Ok(true)
    }

    /// Feed a frame and invoke `f` with each completed UTF-8 JSON document,
    /// reusing `out` as the decompression buffer (zero per-message heap
    /// allocations).
    pub(crate) fn push_each(
        &mut self,
        data: &[u8],
        out: &mut Vec<u8>,
        mut f: impl FnMut(&str),
    ) -> Result<(), GatewayError> {
        if self.try_inflate_single(data, out)? {
            if let Ok(json) = std::str::from_utf8(out) {
                f(json);
            }
            shrink_output(out);
            return Ok(());
        }
        if self.pending.len().saturating_add(data.len()) > self.max_message {
            return Err(GatewayError::zlib("compressed message exceeds size cap"));
        }
        self.pending.extend_from_slice(data);

        let mut start = 0;
        while let Some(rel_end) = find_suffix(&self.pending[start..]) {
            let end = start + rel_end;
            // Inflate directly from the slice without allocating a temporary `Vec<u8>`.
            let res = inflate_slice(
                &mut self.inflate,
                &self.pending[start..=end],
                out,
                self.max_message,
            );
            start = end + 1;
            res?;
            let json = std::str::from_utf8(out)
                .map_err(|_| GatewayError::zlib("decompressed payload is not UTF-8"))?;
            f(json);
        }
        if start > 0 {
            self.pending.drain(..start);
        }
        self.maybe_shrink();
        shrink_output(out);
        Ok(())
    }

    fn inflate_into(&mut self, chunk: &[u8], out: &mut Vec<u8>) -> Result<(), GatewayError> {
        inflate_slice(&mut self.inflate, chunk, out, self.max_message)
    }

    fn maybe_shrink(&mut self) {
        if self.pending.capacity() > SHRINK_AFTER && self.pending.len() < SHRINK_TO {
            self.pending.shrink_to(SHRINK_TO);
        }
        shrink_output(&mut self.output);
    }
}

pub(crate) fn shrink_output(out: &mut Vec<u8>) {
    if out.capacity() > SHRINK_AFTER {
        out.clear();
        out.shrink_to(SHRINK_TO);
    }
}

fn inflate_slice(
    inflate: &mut Decompress,
    chunk: &[u8],
    out: &mut Vec<u8>,
    max_message: usize,
) -> Result<(), GatewayError> {
    out.clear();
    let mut src = chunk;
    loop {
        if out.len() > max_message {
            return Err(GatewayError::zlib("decompressed message exceeds size cap"));
        }
        let in_before = inflate.total_in();
        let status = inflate
            .decompress_vec(src, out, FlushDecompress::Sync)
            .map_err(GatewayError::zlib)?;
        let used = consumed(inflate.total_in(), in_before);
        src = src.get(used..).unwrap_or(&[]);
        match status {
            Status::Ok | Status::StreamEnd | Status::BufError if src.is_empty() => break,
            Status::BufError => out.reserve(4096),
            Status::Ok | Status::StreamEnd => {}
        }
    }
    if out.len() > max_message {
        return Err(GatewayError::zlib("decompressed message exceeds size cap"));
    }
    Ok(())
}

fn consumed(total_in: u64, before: u64) -> usize {
    usize::try_from(total_in.saturating_sub(before)).unwrap_or(usize::MAX)
}

fn find_suffix(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == ZLIB_SUFFIX).map(|i| i + 3)
}

/// Compress one JSON document as a zlib-stream frame (mock gateway).
pub(crate) fn compress_json(json: &[u8]) -> Result<Vec<u8>, GatewayError> {
    use flate2::{Compress, Compression, FlushCompress};

    let mut compress = Compress::new(Compression::fast(), true);
    let mut out = Vec::with_capacity(json.len() + 16);
    let mut src = json;
    loop {
        let in_before = compress.total_in();
        let status = compress
            .compress_vec(src, &mut out, FlushCompress::Sync)
            .map_err(GatewayError::zlib)?;
        let used = consumed(compress.total_in(), in_before);
        src = src.get(used..).unwrap_or(&[]);
        match status {
            Status::Ok | Status::StreamEnd => {
                if src.is_empty() {
                    break;
                }
            }
            Status::BufError => {
                if src.is_empty() {
                    break;
                }
                out.reserve(256);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_one_message() {
        let json = r#"{"op":10,"d":{"heartbeat_interval":45000}}"#;
        let bin = compress_json(json.as_bytes()).unwrap();
        assert!(bin.ends_with(&ZLIB_SUFFIX));
        let mut z = ZlibStream::new();
        let out = z.push(&bin).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(&*out[0], json);
    }

    #[test]
    fn split_across_frames() {
        let json = r#"{"op":11}"#;
        let bin = compress_json(json.as_bytes()).unwrap();
        let mid = bin.len() / 2;
        let mut z = ZlibStream::new();
        assert_eq!(z.push(&bin[..mid]).unwrap().len(), 0);
        let out = z.push(&bin[mid..]).unwrap();
        assert_eq!(&*out[0], json);
    }

    #[test]
    fn two_messages_one_context() {
        use flate2::{Compress, Compression, FlushCompress, Status};

        let a = r#"{"op":10}"#;
        let b = r#"{"op":11}"#;
        let mut compress = Compress::new(Compression::fast(), true);
        let mut frame = |json: &[u8]| {
            let mut out = Vec::new();
            let mut src = json;
            loop {
                let before = compress.total_in();
                let status = compress
                    .compress_vec(src, &mut out, FlushCompress::Sync)
                    .unwrap();
                let used = consumed(compress.total_in(), before);
                src = src.get(used..).unwrap_or(&[]);
                match status {
                    Status::Ok | Status::StreamEnd | Status::BufError if src.is_empty() => break,
                    Status::BufError => out.reserve(256),
                    Status::Ok | Status::StreamEnd => {}
                }
            }
            out
        };
        let mut z = ZlibStream::new();
        let first = z.push(&frame(a.as_bytes())).unwrap();
        let second = z.push(&frame(b.as_bytes())).unwrap();
        assert_eq!(first.len(), 1, "first={first:?}");
        assert_eq!(second.len(), 1, "second={second:?}");
        assert_eq!(&*first[0], a);
        assert_eq!(&*second[0], b);
    }

    #[test]
    fn size_cap() {
        let json = r#"{"op":1}"#;
        let bin = compress_json(json.as_bytes()).unwrap();
        let mut z = ZlibStream::with_limit(1);
        assert!(z.push(&bin).is_err());
    }
}
