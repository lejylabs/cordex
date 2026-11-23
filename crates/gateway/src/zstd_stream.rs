//! zstd-stream decompression for one gateway connection.

use zstd::stream::raw::{Decoder, InBuffer, Operation, OutBuffer};

use crate::error::GatewayError;
use crate::zlib::shrink_output;

/// One persistent zstd decompression context for the lifetime of a WebSocket
/// connection.
pub struct ZstdStream {
    decoder: Decoder<'static>,
    max_message: usize,
}

impl ZstdStream {
    /// New context; every connection (including a resume) starts one.
    pub fn with_limit(max_message: usize) -> Result<Self, GatewayError> {
        let decoder = Decoder::new().map_err(GatewayError::zstd)?;
        Ok(Self {
            decoder,
            max_message,
        })
    }

    /// Decompress one WebSocket binary message into `out` (reusing its
    /// capacity across messages). Discord sends exactly one complete gateway
    /// payload per binary message over a single continuous zstd stream.
    pub(crate) fn decompress_into(
        &mut self,
        data: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), GatewayError> {
        if data.len() > self.max_message {
            return Err(GatewayError::zstd("compressed message exceeds size cap"));
        }
        out.clear();
        let mut in_buf = InBuffer::around(data);
        let mut scratch = [0_u8; 4096];
        loop {
            let mut out_buf = OutBuffer::around(&mut scratch[..]);
            let hint = self
                .decoder
                .run(&mut in_buf, &mut out_buf)
                .map_err(GatewayError::zstd)?;
            let produced = out_buf.as_slice();
            if !produced.is_empty() {
                if out.len().saturating_add(produced.len()) > self.max_message {
                    shrink_output(out);
                    return Err(GatewayError::zstd("decompressed message exceeds size cap"));
                }
                out.extend_from_slice(produced);
            }
            // Done once all input bytes of this WebSocket message are consumed
            // and the decoder no longer emits buffered output.
            if in_buf.pos() >= data.len() && (produced.is_empty() || hint == 0) {
                break;
            }
        }
        let _ = std::str::from_utf8(out)
            .map_err(|_| GatewayError::zstd("decompressed payload is not UTF-8"))?;
        Ok(())
    }
}

/// Compress one JSON document as a single flushed zstd-stream frame
/// (used by the mock gateway and unit tests).
pub(crate) fn compress_json(json: &[u8]) -> Result<Vec<u8>, GatewayError> {
    let mut encoder = zstd::stream::raw::Encoder::new(1).map_err(GatewayError::zstd)?;
    let mut out = Vec::with_capacity(json.len() + 64);
    let mut in_buf = InBuffer::around(json);
    let mut scratch = [0_u8; 4096];
    while in_buf.pos() < json.len() {
        let mut out_buf = OutBuffer::around(&mut scratch[..]);
        encoder
            .run(&mut in_buf, &mut out_buf)
            .map_err(GatewayError::zstd)?;
        out.extend_from_slice(out_buf.as_slice());
    }
    loop {
        let mut out_buf = OutBuffer::around(&mut scratch[..]);
        let remaining = encoder.flush(&mut out_buf).map_err(GatewayError::zstd)?;
        out.extend_from_slice(out_buf.as_slice());
        if remaining == 0 {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompresses_single_message() {
        let json = r#"{"op":0,"t":"READY","s":1,"d":{"v":10}}"#;
        let bin = compress_json(json.as_bytes()).unwrap();
        let mut stream = ZstdStream::with_limit(1024 * 1024).unwrap();
        let mut out = Vec::new();
        stream.decompress_into(&bin, &mut out).unwrap();
        assert_eq!(std::str::from_utf8(&out).unwrap(), json);
    }

    #[test]
    fn decompresses_multiple_messages_in_one_continuous_stream() {
        let mut encoder = zstd::stream::raw::Encoder::new(1).unwrap();
        let mut compress_chunk = |json: &str| {
            let mut out = Vec::new();
            let mut in_buf = InBuffer::around(json.as_bytes());
            let mut scratch = [0_u8; 4096];
            while in_buf.pos() < json.len() {
                let mut out_buf = OutBuffer::around(&mut scratch[..]);
                encoder.run(&mut in_buf, &mut out_buf).unwrap();
                out.extend_from_slice(out_buf.as_slice());
            }
            loop {
                let mut out_buf = OutBuffer::around(&mut scratch[..]);
                let rem = encoder.flush(&mut out_buf).unwrap();
                out.extend_from_slice(out_buf.as_slice());
                if rem == 0 {
                    break;
                }
            }
            out
        };

        let first = r#"{"op":0,"t":"MESSAGE_CREATE","s":1,"d":{"content":"hello world"}}"#;
        let second = r#"{"op":0,"t":"MESSAGE_CREATE","s":2,"d":{"content":"hello world again"}}"#;
        let c1 = compress_chunk(first);
        let c2 = compress_chunk(second);

        let mut stream = ZstdStream::with_limit(1024 * 1024).unwrap();
        let mut out = Vec::new();
        stream.decompress_into(&c1, &mut out).unwrap();
        assert_eq!(std::str::from_utf8(&out).unwrap(), first);
        stream.decompress_into(&c2, &mut out).unwrap();
        assert_eq!(std::str::from_utf8(&out).unwrap(), second);
    }

    #[test]
    fn rejects_payload_exceeding_size_cap() {
        let json = format!(r#"{{"d":"{}"}}"#, "a".repeat(2048));
        let bin = compress_json(json.as_bytes()).unwrap();
        let mut stream = ZstdStream::with_limit(256).unwrap();
        let mut out = Vec::new();
        let err = stream.decompress_into(&bin, &mut out).unwrap_err();
        assert!(err.to_string().contains("size cap"), "{err}");
    }
}
