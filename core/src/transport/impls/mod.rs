#[cfg(any(feature = "tungstenite", feature = "tokio-tungstenite"))]
pub(crate) mod common;

#[cfg(feature = "reqwest-transport")]
mod reqwest;
#[cfg(feature = "tungstenite")]
mod tungstenite;
#[cfg(feature = "tokio-tungstenite")]
mod tokio_tungstenite;

#[cfg(feature = "reqwest-transport")]
pub use self::reqwest::ReqwestTransport;
#[cfg(feature = "tungstenite")]
pub use self::tungstenite::WsTransport;
#[cfg(feature = "tokio-tungstenite")]
pub use self::tokio_tungstenite::AsyncWsTransport;
