use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::Path;

use crate::utils::detect_mimetype;

/// A blob of non-text content attached to or emitted by a message.
///
/// Mimetype is the single source of truth for *what* the bytes mean.
/// `source` is *how* to retrieve them. `meta` is a free-form bag for
/// provider-specific annotations — consumers that don't care ignore it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct File {
    pub mime: MimeType,
    pub source: FileSource,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub meta: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum FileSource {
    Bytes(Vec<u8>),
    Url(url::Url),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MimeType(String);

impl MimeType {
    pub fn from(s: impl Into<String>) -> MimeType {
        MimeType(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl File {
    /// Construct a file from raw bytes, sniffing the mimetype from content.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> File {
        let bytes = bytes.into();
        let mime = MimeType(detect_mimetype(&bytes));
        File {
            mime,
            source: FileSource::Bytes(bytes),
            meta: Value::Null,
        }
    }

    /// Construct a file from raw bytes with an explicit mimetype.
    pub fn from_bytes_with_mime(bytes: impl Into<Vec<u8>>, mime: impl Into<String>) -> File {
        File {
            mime: MimeType(mime.into()),
            source: FileSource::Bytes(bytes.into()),
            meta: Value::Null,
        }
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<File, std::io::Error> {
        let bytes = fs::read(path.as_ref())?;
        Ok(File::from_bytes(bytes))
    }

    /// Parse a URL. Mimetype defaults to `application/octet-stream` when
    /// unknown — callers can supply a hint explicitly.
    pub fn from_url(
        url: impl Into<String>,
        mime: Option<&str>,
    ) -> Result<File, Box<dyn std::error::Error + Send + Sync>> {
        let url = url::Url::parse(&url.into())?;
        let mime = MimeType(
            mime.map(str::to_string)
                .unwrap_or_else(|| "application/octet-stream".to_string()),
        );
        Ok(File {
            mime,
            source: FileSource::Url(url),
            meta: Value::Null,
        })
    }

    pub fn with_meta(mut self, meta: Value) -> Self {
        self.meta = meta;
        self
    }

    pub fn is_image(&self) -> bool {
        self.mime.as_str().starts_with("image/")
    }

    pub fn is_audio(&self) -> bool {
        self.mime.as_str().starts_with("audio/")
    }

    pub fn is_video(&self) -> bool {
        self.mime.as_str().starts_with("video/")
    }

    /// Anything that isn't image/audio/video is treated as a document.
    pub fn is_document(&self) -> bool {
        !self.is_image() && !self.is_audio() && !self.is_video()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_bytes_detects_png_mime() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let file = File::from_bytes(png.to_vec());
        assert_eq!(file.mime.as_str(), "image/png");
        assert!(file.is_image());
    }

    #[test]
    fn from_url_defaults_to_octet_stream() {
        let file = File::from_url("https://example.com/x", None).unwrap();
        assert_eq!(file.mime.as_str(), "application/octet-stream");
        assert!(file.is_document());
    }

    #[test]
    fn classification_helpers() {
        let img = File::from_bytes_with_mime(vec![], "image/jpeg");
        let aud = File::from_bytes_with_mime(vec![], "audio/mp3");
        let vid = File::from_bytes_with_mime(vec![], "video/mp4");
        let pdf = File::from_bytes_with_mime(vec![], "application/pdf");

        assert!(img.is_image() && !img.is_audio());
        assert!(aud.is_audio() && !aud.is_image());
        assert!(vid.is_video() && !vid.is_document());
        assert!(pdf.is_document() && !pdf.is_image());
    }

    #[test]
    fn with_meta_populates_bag() {
        let file = File::from_bytes_with_mime(b"x".to_vec(), "image/png")
            .with_meta(json!({ "openai_file_id": "file_abc" }));
        assert_eq!(file.meta["openai_file_id"], "file_abc");
    }

    #[test]
    fn meta_null_is_omitted_on_serialize() {
        let file = File::from_bytes_with_mime(b"x".to_vec(), "image/png");
        let json = serde_json::to_string(&file).unwrap();
        assert!(!json.contains("meta"), "null meta should be skipped: {json}");
    }

    #[test]
    fn serde_roundtrip_with_meta() {
        let file = File::from_bytes_with_mime(b"hi".to_vec(), "image/png")
            .with_meta(json!({ "k": 1 }));
        let s = serde_json::to_string(&file).unwrap();
        let back: File = serde_json::from_str(&s).unwrap();
        assert_eq!(file, back);
    }
}
