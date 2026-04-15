mod impls;
pub mod sse;
mod traits;
mod types;

pub use traits::Transport;
pub use types::{Event, EventStream, Request, Response, TransportError};

#[cfg(feature = "tokio-tungstenite")]
pub use impls::AsyncWsTransport;
#[cfg(feature = "reqwest-transport")]
pub use impls::ReqwestTransport;
#[cfg(feature = "tungstenite")]
pub use impls::WsTransport;
