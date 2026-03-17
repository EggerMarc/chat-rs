pub mod macros;
mod router;
pub mod strategy;
pub use router::Router;
pub use strategy::{RoutingStrategy, StrategyError};

use std::marker::PhantomData;

use chat_core::traits::CompletionProvider;

pub struct WithoutProvider;
pub struct WithProvider;

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
