use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum File {
    Url(UrlData),
    Bytes(RawData),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RawData {
    pub mimetype: MimeType,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UrlData {
    pub url: Url,
    pub mimetype: Option<MimeType>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MimeType(String);

impl FromStr for UrlData {
    type Err = Box<dyn std::error::Error>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s)?;
        Ok(UrlData {
            url,
            mimetype: None,
        })
    }
}

impl UrlData {
    pub fn with_mimetype(&mut self, mimetype: MimeType) -> &mut Self {
        self.mimetype = Some(mimetype);
        self
    }

    pub fn from(
        url: impl Into<String>,
        mimetype: impl Into<String>,
    ) -> Result<UrlData, Box<dyn std::error::Error>> {
        let url = Url::parse(&url.into())?;
        let mimetype = MimeType(mimetype.into());
        Ok(UrlData {
            url,
            mimetype: Some(mimetype),
        })
    }
}

impl RawData {
    pub fn from(raw: impl Into<Vec<u8>>, mimetype: impl Into<String>) -> RawData {
        let bytes = raw.into();
        let mimetype = MimeType(mimetype.into());
        RawData { bytes, mimetype }
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl File {
    pub fn from_bytes(bytes: impl Into<Vec<u8>>, mimetype: impl Into<String>) -> File {
        File::Bytes(RawData::from(bytes, mimetype))
    }

    pub fn from_url(
        url: impl Into<String>,
        mimetype: Option<&str>,
    ) -> Result<File, Box<dyn std::error::Error>> {
        let url = if let Some(mimetype) = mimetype {
            UrlData::from(url, mimetype)?
        } else {
            UrlData::from_str(&url.into())?
        };
        Ok(File::Url(url))
    }
}
