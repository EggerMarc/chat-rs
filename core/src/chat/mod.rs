use tools_rs::ToolCollection;

use crate::{
    chat::state::Unstructured,
    error::ChatError,
    types::{
        callback::{CallbackStrategy, RetryStrategy},
        messages::{
            content::Content,
            parts::{PartEnum, Parts},
        },
        options::ChatOptions,
    },
};

pub mod completion;
pub mod embed;
pub mod state;
#[cfg(feature = "stream")]
pub mod stream;

#[derive(Default)]
pub struct Chat<CP, Output = Unstructured> {
    pub(crate) model: CP,
    pub(crate) output_shape: Option<schemars::Schema>,
    pub(crate) model_options: Option<ChatOptions>,
    pub(crate) max_steps: Option<u16>,
    pub(crate) max_retries: Option<u16>,
    pub(crate) retry_strategy: Option<RetryStrategy>,
    pub(crate) before_strategy: Option<CallbackStrategy>,
    pub(crate) after_strategy: Option<CallbackStrategy>,
    pub(crate) tools: Option<ToolCollection>,
    pub(crate) _output: std::marker::PhantomData<Output>,
}

impl<P, Output> Chat<P, Output> {
    pub(crate) async fn tool_call(&self, content: &Content) -> Result<Parts, ChatError> {
        let mut frs: Parts = Parts::default();
        for fc in content.parts.function_calls() {
            frs.push(PartEnum::from_function_response(
                self.tools
                    .as_ref()
                    .ok_or(ChatError::InvalidResponse(
                        "Attempted to call tool but no tool collection has been set.".to_string(),
                    ))?
                    .call(fc.clone())
                    .await
                    .map_err(|_err| ChatError::InvalidResponse("Tools error".to_string()))?,
            ));
        }
        Ok(frs)
    }
}
