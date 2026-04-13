mod traits;
mod types;
pub mod sse;

#[cfg(feature = "reqwest-transport")]
mod reqwest;
#[cfg(any(feature = "tungstenite", feature = "tokio-tungstenite"))]
mod ws_common;
#[cfg(feature = "tungstenite")]
mod ws_tungstenite;
#[cfg(feature = "tokio-tungstenite")]
mod ws_tokio_tungstenite;

pub use traits::Transport;
pub use types::{Event, EventStream, Request, Response, TransportError};

#[cfg(feature = "reqwest-transport")]
pub use self::reqwest::ReqwestTransport;
#[cfg(feature = "tungstenite")]
pub use self::ws_tungstenite::WsTransport;
#[cfg(feature = "tokio-tungstenite")]
pub use self::ws_tokio_tungstenite::AsyncWsTransport;
