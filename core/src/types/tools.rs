//! Type-erased tool collections with user-defined execution strategies.
//!
//! The chat loop holds [`Vec<Box<dyn TypedCollection>>`] and routes each
//! model-emitted tool call to the collection that owns its name. Each
//! collection wraps a typed `tools_rs::ToolCollection<M>` together with a
//! user-supplied strategy closure that inspects the call + its metadata
//! and returns an [`Action`] — the chat-rs vocabulary for loop-flow
//! decisions (execute now, require human approval, defer to a later time,
//! or reject outright).
//!
//! The typical wiring:
//!
//! ```ignore
//! let raw = ToolCollection::<MyPolicy>::collect_tools()?;
//! let scoped = ScopedCollection::new(raw, |call, meta| {
//!     if meta.destructive { Action::RequireApproval }
//!     else { Action::Execute }
//! });
//! let chat = ChatBuilder::new()
//!     .with_scoped_tools(scoped)
//!     .build();
//! ```
//!
//! Strategies are arbitrary closures, not trait impls — they can close over
//! feature flags, rate-limit buckets, user sessions, or anything else the
//! host application needs to decide how a tool should be run.

use std::future::Future;
use std::pin::Pin;

use serde_json::Value;
use tools_core::NoMeta;
use tools_rs::{FunctionCall, FunctionResponse, ToolCollection, ToolError};

/// The chat loop's decision vocabulary for what to do with a model-emitted
/// tool call before it runs. Produced by a strategy closure, consumed by
/// the executor.
///
/// Every variant is a loop-flow decision; nothing in this enum influences
/// what the tool itself does when executed. That's deliberate — metadata
/// and strategies are only ever allowed to change how the loop interacts
/// with the tool, never the tool's behavior.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Action {
    /// Run the tool immediately.
    Execute,

    /// Leave the tool [`ToolStatus::Pending`] and pause the loop so a
    /// human can approve, reject, or edit it before execution.
    RequireApproval,

    /// Defer execution. The executor marks the tool with its current
    /// status and pauses the loop; the caller is expected to persist
    /// the conversation and resume at or after `at`.
    ///
    /// Currently surfaces as a pause signal with timing metadata. Full
    /// scheduled-execution support (dedicated `ToolStatus::Scheduled`
    /// variant, wake-up orchestration) is a future extension.
    Defer { at: std::time::SystemTime },

    /// Decline the call. Surfaces to the model via the synthesized
    /// error response that [`ToolStatus::Rejected`] produces on the
    /// wire, so the model can react.
    Reject { reason: String },
}

/// Type-erased view of a tool collection used by the chat loop.
///
/// Implementations wrap a typed `ToolCollection<M>` + a decision strategy.
/// The `M` disappears behind the trait object; the chat loop works
/// uniformly over any collection that speaks this interface.
pub trait TypedCollection: Send + Sync {
    /// Every tool name this collection owns. Used at builder time to
    /// populate the routing table and detect collisions with other
    /// collections.
    fn names(&self) -> Vec<&'static str>;

    /// Tool declarations as the providers need them, in the same shape
    /// `ToolCollection::json` returns today.
    fn declarations(&self) -> Result<Value, ToolError>;

    /// Ask the strategy what to do with this call. Pure decision — no
    /// side effects.
    fn decide(&self, call: &FunctionCall) -> Action;

    /// Actually execute the call. Called by the executor once a tool has
    /// been approved (auto or manually).
    fn call<'a>(
        &'a self,
        call: FunctionCall,
    ) -> Pin<Box<dyn Future<Output = Result<FunctionResponse, ToolError>> + Send + 'a>>;
}

/// A [`ToolCollection<M>`] paired with a user-defined strategy closure.
///
/// Boxed into `dyn TypedCollection` when handed to the chat builder — the
/// generic `M` and `F` disappear at that boundary.
pub struct ScopedCollection<M, F> {
    tools: ToolCollection<M>,
    strategy: F,
}

impl<M, F> ScopedCollection<M, F>
where
    M: Send + Sync + 'static,
    F: Fn(&FunctionCall, &M) -> Action + Send + Sync + 'static,
{
    /// Wrap a typed tool collection with a decision strategy.
    pub fn new(tools: ToolCollection<M>, strategy: F) -> Self {
        Self { tools, strategy }
    }
}

impl ScopedCollection<NoMeta, fn(&FunctionCall, &NoMeta) -> Action> {
    /// Shortcut for the common case: wrap an unmetadata'd collection with
    /// an always-execute strategy. Replaces the old
    /// `ChatBuilder::with_tools(collection)` convenience path.
    pub fn auto_execute(
        tools: ToolCollection<NoMeta>,
    ) -> ScopedCollection<NoMeta, fn(&FunctionCall, &NoMeta) -> Action> {
        fn always_execute(_: &FunctionCall, _: &NoMeta) -> Action {
            Action::Execute
        }
        ScopedCollection {
            tools,
            strategy: always_execute,
        }
    }
}

impl<M, F> TypedCollection for ScopedCollection<M, F>
where
    M: Send + Sync + 'static,
    F: Fn(&FunctionCall, &M) -> Action + Send + Sync + 'static,
{
    fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|(name, _)| name).collect()
    }

    fn declarations(&self) -> Result<Value, ToolError> {
        self.tools.json()
    }

    fn decide(&self, call: &FunctionCall) -> Action {
        match self.tools.meta(&call.name) {
            Some(meta) => (self.strategy)(call, meta),
            // Tool isn't in this collection — router shouldn't have sent
            // it here. Fail-safe: require approval so the mistake surfaces.
            None => Action::RequireApproval,
        }
    }

    fn call<'a>(
        &'a self,
        call: FunctionCall,
    ) -> Pin<Box<dyn Future<Output = Result<FunctionResponse, ToolError>> + Send + 'a>> {
        Box::pin(self.tools.call(call))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use NoMeta;

    fn fc(name: &str) -> FunctionCall {
        FunctionCall {
            id: Some(tools_rs::CallId::new()),
            name: name.to_string(),
            arguments: json!({}),
        }
    }

    #[test]
    fn auto_execute_always_returns_execute() {
        let coll: ToolCollection<NoMeta> = ToolCollection::default();
        let scoped = ScopedCollection::auto_execute(coll);
        let erased: Box<dyn TypedCollection> = Box::new(scoped);
        // No tool registered, so decide() falls through to the safe default.
        let action = erased.decide(&fc("anything"));
        assert!(matches!(action, Action::RequireApproval));
    }

    #[test]
    fn custom_strategy_reads_metadata() {
        #[derive(Clone, serde::Deserialize, Default)]
        struct MyMeta {
            danger: bool,
        }

        let mut coll: ToolCollection<MyMeta> = ToolCollection::new();
        coll.register(
            "zap",
            "zap a thing",
            |_: String| async { "done".to_string() },
            MyMeta { danger: true },
        )
        .unwrap();

        let scoped = ScopedCollection::new(coll, |_call, meta: &MyMeta| {
            if meta.danger {
                Action::RequireApproval
            } else {
                Action::Execute
            }
        });

        let erased: Box<dyn TypedCollection> = Box::new(scoped);
        let action = erased.decide(&fc("zap"));
        assert!(matches!(action, Action::RequireApproval));
    }
}
