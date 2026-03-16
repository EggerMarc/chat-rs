use chat_core::types::messages::Messages;

/// Determines the order in which providers are tried.
///
/// Given the current messages and the number of available providers,
/// returns a `Vec` of provider indices in the order they should be attempted.
pub trait RoutingStrategy: Send + Sync {
    fn rank(&self, messages: &Messages, count: usize) -> Vec<usize>;
}
