//! zlib-stream inflate for one gateway connection (PLAN.md §8.10).

use flate2::{Decompress, FlushDecompress, Status};

use crate::error::GatewayError;

/// Suffix Discord writes after each zlib-stream payload (`Z_SYNC_FLUSH`).
pub(crate) const ZLIB_SUFFIX: [u8; 4] = [0, 0, 0xFF, 0xFF];

/// Default cap on one decompressed gateway message.
pub(crate) const MAX_MESSAGE: usize = 16 * 1024 * 1024;

const SHRINK_AFTER: usize = 1024 * 1024;
const SHRINK_TO: usize = 8 * 1024;

/// One inflate context for the lifetime of a socket.
pub(crate) struct ZlibStream {
    inflate: Decompress,
    pending: Vec<u8>,
    output: Vec<u8>,
    max_message: usize,
}

impl ZlibStream {
    /// New context; every connection (including a resume) starts one.
    pub(crate) fn new() -> Self {
        Self::with_limit(MAX_MESSAGE)
    }

    pub(crate) fn with_limit(max_message: usize) -> Self {
        Self {
            inflate: Decompress::new(true),
            pending: Vec::with_capacity(4096),
            output: Vec::with_capacity(4096),
            max_message,
        }
    }

    /// Feed a WebSocket binary frame. Completed JSON documents are returned
    /// in order (a frame may contain several, or only part of one).
    pub(crate) fn push(&mut self, data: &[u8]) -> Result<Vec<Box<str>>, GatewayError> {
        if self.pending.len().saturating_add(data.len()) > self.max_message {
            return Err(GatewayError::zlib("compressed message exceeds size cap"));
        }
        self.pending.extend_from_slice(data);

        let mut messages = Vec::new();
        while let Some(end) = find_suffix(&self.pending) {
            let chunk: Vec<u8> = self.pending.drain(..=end).collect();
            messages.push(self.inflate_chunk(&chunk)?);
        }
        self.maybe_shrink();
        Ok(messages)
    }

    fn inflate_chunk(&mut self, chunk: &[u8]) -> Result<Box<str>, GatewayError> {
        self.output.clear();
        let mut src = chunk;
        loop {
            if self.output.len() > self.max_message {
                return Err(GatewayError::zlib("decompressed message exceeds size cap"));
            }
            let in_before = self.inflate.total_in();
            let status = self
                .inflate
                .decompress_vec(src, &mut self.output, FlushDecompress::Sync)
                .map_err(GatewayError::zlib)?;
            let used = consumed(self.inflate.total_in(), in_before);
            src = src.get(used..).unwrap_or(&[]);
            match status {
                Status::Ok | Status::StreamEnd | Status::BufError if src.is_empty() => break,
                Status::BufError => self.output.reserve(4096),
                Status::Ok | Status::StreamEnd => {}
            }
        }
        if self.output.len() > self.max_message {
            return Err(GatewayError::zlib("decompressed message exceeds size cap"));
        }
        let json = std::str::from_utf8(&self.output)
            .map_err(|_| GatewayError::zlib("decompressed payload is not UTF-8"))?;
        Ok(json.into())
    }

    fn maybe_shrink(&mut self) {
        if self.pending.capacity() > SHRINK_AFTER && self.pending.len() < SHRINK_TO {
            self.pending.shrink_to(SHRINK_TO);
        }
        if self.output.capacity() > SHRINK_AFTER {
            self.output.clear();
            self.output.shrink_to(SHRINK_TO);
        }
    }
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
