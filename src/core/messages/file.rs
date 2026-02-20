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
    /// Parses a string as a URL and constructs a `UrlData` with no MIME type.
    ///
    /// Returns an error if the input is not a valid URL.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// let data = UrlData::from_str("https://example.com").unwrap();
    /// assert!(data.url.as_str().starts_with("https://example.com"));
    /// assert!(data.mimetype.is_none());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(s)?;
        Ok(UrlData {
            url,
            mimetype: None,
        })
    }
}

impl UrlData {
    /// Set the URL data's MIME type and return a mutable reference to the updated value.
    ///
    /// # Returns
    ///
    /// `&mut Self` with `mimetype` set to the provided value.
    ///
    /// # Examples
    ///
    /// ```
    /// use reqwest::Url;
    ///
    /// let mut ud = crate::UrlData { url: Url::parse("https://example.com").unwrap(), mimetype: None };
    /// ud.with_mimetype(crate::MimeType("text/plain".into()));
    /// assert_eq!(ud.mimetype.unwrap().to_string(), "text/plain");
    /// ```
    pub fn with_mimetype(&mut self, mimetype: MimeType) -> &mut Self {
        self.mimetype = Some(mimetype);
        self
    }

    /// Creates a `UrlData` from a URL string and a MIME type.
    ///
    /// The resulting `UrlData` contains the parsed `Url` and the provided MIME type.
    /// Returns an error if the URL string cannot be parsed.
    ///
    /// # Examples
    ///
    /// ```
    /// let ud = UrlData::from("https://example.com/file.png", "image/png").unwrap();
    /// assert_eq!(ud.url.as_str(), "https://example.com/file.png");
    /// assert_eq!(ud.mimetype.unwrap().to_string(), "image/png");
    /// ```
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
    /// Constructs a RawData value from raw bytes and a MIME type.
    ///
    /// `raw` can be any value convertible into `Vec<u8>`; `mimetype` can be any value convertible into `String`.
    ///
    /// # Returns
    ///
    /// A `RawData` containing the provided bytes and the given MIME type.
    ///
    /// # Examples
    ///
    /// ```
    /// let data = RawData::from(b"hello".to_vec(), "text/plain");
    /// assert_eq!(data.bytes, b"hello".to_vec());
    /// assert_eq!(data.mimetype.to_string(), "text/plain");
    /// ```
    pub fn from(raw: impl Into<Vec<u8>>, mimetype: impl Into<String>) -> RawData {
        let bytes = raw.into();
        let mimetype = MimeType(mimetype.into());
        RawData { bytes, mimetype }
    }
}

impl fmt::Display for MimeType {
    /// Formats the `MimeType` by writing its inner string representation.
    ///
    /// # Examples
    ///
    /// ```
    /// let mt = MimeType("text/plain".into());
    /// assert_eq!(format!("{}", mt), "text/plain");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl File {
    /// Creates a `File` containing raw bytes with the specified MIME type.
    ///
    /// # Parameters
    ///
    /// - `bytes`: Raw byte content for the file.
    /// - `mimetype`: MIME type string describing the bytes (e.g., `"image/png"`, `"text/plain"`).
    ///
    /// # Returns
    ///
    /// A `File::Bytes` variant wrapping a `RawData` that holds the provided bytes and MIME type.
    ///
    /// # Examples
    ///
    /// ```
    /// let f = File::from_bytes(b"hello".to_vec(), "text/plain");
    /// match f {
    ///     File::Bytes(raw) => {
    ///         assert_eq!(raw.mimetype.to_string(), "text/plain");
    ///         assert_eq!(raw.bytes, b"hello".to_vec());
    ///     }
    ///     _ => panic!("expected Bytes variant"),
    /// }
    /// ```
    pub fn from_bytes(bytes: impl Into<Vec<u8>>, mimetype: impl Into<String>) -> File {
        File::Bytes(RawData::from(bytes, mimetype))
    }

    /// Parses a URL string and returns a `File::Url` containing the parsed `UrlData`, optionally with the provided MIME type.
    ///
    /// If `mimetype` is `Some`, the returned `UrlData` will include that MIME type; if `None`, the `UrlData` will have no MIME type set.
    ///
    /// # Errors
    ///
    /// Returns an `Err` if the provided URL string cannot be parsed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use core::messages::file::File;
    /// // with explicit mimetype
    /// let f = File::from_url("https://example.com/image.png", Some("image/png")).unwrap();
    ///
    /// // without mimetype
    /// let f2 = File::from_url("https://example.com/document.pdf", None).unwrap();
    /// ```
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