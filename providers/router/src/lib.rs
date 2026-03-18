mod router;
pub mod strategy;
pub use router::Router;
pub use strategy::{RoutingStrategy, StrategyError};

#[cfg(feature = "stream")]
mod stream_router;
#[cfg(feature = "stream")]
pub use stream_router::StreamRouter;

use std::marker::PhantomData;

use chat_core::traits::CompletionProvider;

#[cfg(feature = "stream")]
use chat_core::traits::ChatProvider;

pub struct WithoutProvider;
pub struct WithProvider;

// ---------------------------------------------------------------------------
// RouterBuilder — completion-only router
// ---------------------------------------------------------------------------

pub struct RouterBuilder<M = WithoutProvider> {
    providers: Vec<Box<dyn CompletionProvider>>,
    strategy: Option<Box<dyn RoutingStrategy>>,
    _m: PhantomData<M>,
}

impl Default for RouterBuilder<WithoutProvider> {
    fn default() -> Self {
        Self::new()
    }
}

impl RouterBuilder<WithoutProvider> {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            strategy: None,
            _m: PhantomData,
        }
    }

    pub fn add_provider(
        self,
        provider: impl CompletionProvider + 'static,
    ) -> RouterBuilder<WithProvider> {
        let mut providers = self.providers;
        providers.push(Box::new(provider));
        RouterBuilder {
            providers,
            strategy: self.strategy,
            _m: PhantomData,
        }
    }
}

impl RouterBuilder<WithProvider> {
    pub fn add_provider(mut self, provider: impl CompletionProvider + 'static) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    pub fn with_strategy(mut self, strategy: impl RoutingStrategy + 'static) -> Self {
        self.strategy = Some(Box::new(strategy));
        self
    }

    pub fn build(self) -> Router {
        Router {
            providers: self.providers,
            strategy: self.strategy,
        }
    }
}

// ---------------------------------------------------------------------------
// StreamRouterBuilder — streaming router (all providers must support stream)
// ---------------------------------------------------------------------------

#[cfg(feature = "stream")]
pub struct StreamRouterBuilder<M = WithoutProvider> {
    providers: Vec<Box<dyn ChatProvider>>,
    strategy: Option<Box<dyn RoutingStrategy>>,
    _m: PhantomData<M>,
}

#[cfg(feature = "stream")]
impl Default for StreamRouterBuilder<WithoutProvider> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "stream")]
impl StreamRouterBuilder<WithoutProvider> {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            strategy: None,
            _m: PhantomData,
        }
    }

    pub fn add_provider(
        self,
        provider: impl ChatProvider + 'static,
    ) -> StreamRouterBuilder<WithProvider> {
        let mut providers = self.providers;
        providers.push(Box::new(provider));
        StreamRouterBuilder {
            providers,
            strategy: self.strategy,
            _m: PhantomData,
        }
    }
}

#[cfg(feature = "stream")]
impl StreamRouterBuilder<WithProvider> {
    pub fn add_provider(mut self, provider: impl ChatProvider + 'static) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    pub fn with_strategy(mut self, strategy: impl RoutingStrategy + 'static) -> Self {
        self.strategy = Some(Box::new(strategy));
        self
    }

    pub fn build(self) -> StreamRouter {
        StreamRouter {
            providers: self.providers,
            strategy: self.strategy,
            last_used: None,
        }
    }
}
