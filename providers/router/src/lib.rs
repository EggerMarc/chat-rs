mod circuit_breaker;
mod router;
pub mod strategy;
pub use circuit_breaker::CircuitBreakerConfig;
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

use crate::circuit_breaker::CircuitBreaker;

pub struct WithoutProvider;
pub struct WithProvider;

// ---------------------------------------------------------------------------
// RouterBuilder — completion-only router
// ---------------------------------------------------------------------------

pub struct RouterBuilder<M = WithoutProvider> {
    providers: Vec<Box<dyn CompletionProvider>>,
    strategy: Option<Box<dyn RoutingStrategy>>,
    circuit_breaker_config: Option<CircuitBreakerConfig>,
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
            circuit_breaker_config: None,
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
            circuit_breaker_config: self.circuit_breaker_config,
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

    pub fn circuit_breaker(mut self, config: CircuitBreakerConfig) -> Self {
        self.circuit_breaker_config = Some(config);
        self
    }

    pub fn build(self) -> Router {
        let count = self.providers.len();
        Router {
            providers: self.providers,
            strategy: self.strategy,
            circuit_breaker: self
                .circuit_breaker_config
                .map(|config| CircuitBreaker::new(config, count)),
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
    circuit_breaker_config: Option<CircuitBreakerConfig>,
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
            circuit_breaker_config: None,
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
            circuit_breaker_config: self.circuit_breaker_config,
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

    pub fn circuit_breaker(mut self, config: CircuitBreakerConfig) -> Self {
        self.circuit_breaker_config = Some(config);
        self
    }

    pub fn build(self) -> StreamRouter {
        let count = self.providers.len();
        StreamRouter {
            providers: self.providers,
            strategy: self.strategy,
            circuit_breaker: self
                .circuit_breaker_config
                .map(|config| CircuitBreaker::new(config, count)),
            last_used: None,
        }
    }
}
