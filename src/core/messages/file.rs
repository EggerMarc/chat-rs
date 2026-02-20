use reqwest::Url;
use serde::{Deserialize, Serialize};

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

impl UrlData {
    pub fn from_str(value: impl Into<String>) -> Result<UrlData, Box<dyn std::error::Error>> {
        let url = Url::parse(&value.into())?;
        Ok(UrlData {
            url,
            mimetype: None,
        })
    }

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

impl MimeType {
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}
