//! `multipart/form-data` for Discord uploads (PLAN.md §6.2).
//!
//! Discord expects `payload_json` plus `files[n]`, with matching entries in
//! the JSON `attachments` array (`id` = index).

use bytes::{Bytes, BytesMut};
use http::HeaderValue;

use crate::error::HttpError;

/// One file to upload (`files[n]`).
#[derive(Clone, Debug)]
pub struct File {
    /// File name sent to Discord (`SPOILER_` prefix marks a spoiler).
    pub filename: String,
    /// File bytes.
    pub data: Bytes,
    /// MIME type; `application/octet-stream` when omitted.
    pub content_type: Option<String>,
    /// Alt text (attachment `description`).
    pub description: Option<String>,
}

impl File {
    /// File from an in-memory buffer.
    pub fn bytes(filename: impl Into<String>, data: impl Into<Bytes>) -> Self {
        Self {
            filename: filename.into(),
            data: data.into(),
            content_type: None,
            description: None,
        }
    }

    /// Sets `Content-Type` for this part.
    #[must_use]
    pub fn content_type(mut self, mime: impl Into<String>) -> Self {
        self.content_type = Some(mime.into());
        self
    }

    /// Attachment description (alt text).
    #[must_use]
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Prefixes the file name with `SPOILER_` when missing.
    #[must_use]
    pub fn spoiler(mut self) -> Self {
        if !self.filename.starts_with("SPOILER_") {
            self.filename.insert_str(0, "SPOILER_");
        }
        self
    }
}

/// Encoded `multipart/form-data` body.
pub struct Multipart {
    /// JSON payload (`payload_json` part).
    pub payload_json: Bytes,
    /// Files in `files[0]`, `files[1]`, …
    pub files: Vec<File>,
}

impl Multipart {
    /// Builds the body and `Content-Type` header (with boundary).
    pub fn encode(&self) -> Result<(HeaderValue, Bytes), HttpError> {
        let boundary = format!("----cordex{}", fastrand::u64(..));
        let mut out = BytesMut::new();
        write_part(
            &mut out,
            &boundary,
            "payload_json",
            None,
            Some("application/json"),
            &self.payload_json,
        );
        for (i, file) in self.files.iter().enumerate() {
            let name = format!("files[{i}]");
            let ct = file
                .content_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            write_part(
                &mut out,
                &boundary,
                &name,
                Some(&file.filename),
                Some(ct),
                &file.data,
            );
        }
        out.extend_from_slice(b"--");
        out.extend_from_slice(boundary.as_bytes());
        out.extend_from_slice(b"--\r\n");

        let header = format!("multipart/form-data; boundary={boundary}");
        let value = HeaderValue::from_str(&header)
            .map_err(|_| HttpError::validation("multipart boundary is not a valid header"))?;
        Ok((value, out.freeze()))
    }
}

fn write_part(
    out: &mut BytesMut,
    boundary: &str,
    name: &str,
    filename: Option<&str>,
    content_type: Option<&str>,
    data: &[u8],
) {
    out.extend_from_slice(b"--");
    out.extend_from_slice(boundary.as_bytes());
    out.extend_from_slice(b"\r\nContent-Disposition: form-data; name=\"");
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(b"\"");
    if let Some(filename) = filename {
        out.extend_from_slice(b"; filename=\"");
        for b in filename.bytes() {
            if b == b'"' || b == b'\\' {
                out.extend_from_slice(br"\");
            }
            out.extend_from_slice(&[b]);
        }
        out.extend_from_slice(b"\"");
    }
    out.extend_from_slice(b"\r\n");
    if let Some(ct) = content_type {
        out.extend_from_slice(b"Content-Type: ");
        out.extend_from_slice(ct.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(data);
    out.extend_from_slice(b"\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_payload_and_file() {
        let form = Multipart {
            payload_json: Bytes::from(r#"{"content":"hi"}"#),
            files: vec![File::bytes("a.txt", &b"hello"[..]).content_type("text/plain")],
        };
        let (ct, body) = form.encode().unwrap();
        let ct = ct.to_str().unwrap();
        assert!(ct.starts_with("multipart/form-data; boundary="));
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("name=\"payload_json\""));
        assert!(body.contains("name=\"files[0]\""));
        assert!(body.contains("filename=\"a.txt\""));
        assert!(body.contains("hello"));
        assert!(body.contains(r#"{"content":"hi"}"#));
        assert!(body.contains("Content-Type: application/json"));
        assert!(body.contains("Content-Type: text/plain"));
    }

    #[test]
    fn spoiler_prefixes_filename() {
        let file = File::bytes("cat.png", Bytes::new()).spoiler();
        assert_eq!(file.filename, "SPOILER_cat.png");
        let twice = file.spoiler();
        assert_eq!(twice.filename, "SPOILER_cat.png");
    }
}
