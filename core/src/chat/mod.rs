use tools_rs::ToolCollection;

use crate::{
    chat::state::Unstructured,
    error::ChatError,
    types::{
        callback::{CallbackStrategy, RetryStrategy},
        messages::content::Content,
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
    /// Execute any tools in the given `Content` in place, flipping their
    /// status from `Pending`/`Approved` through `Running` to `Completed` or
    /// `Failed`.
    ///
    /// Returns `Ok(true)` if at least one tool was executed. `Rejected` and
    /// already-resolved tools are skipped — they are left alone so that
    /// human-in-the-loop flows can preserve their state through the loop.
    pub(crate) async fn tool_call(&self, content: &mut Content) -> Result<bool, ChatError> {
        let tools = self.tools.as_ref().ok_or_else(|| {
            ChatError::InvalidResponse(
                "Attempted to call tool but no tool collection has been set.".to_string(),
            )
        })?;

        let mut executed = false;

        for tool in content.parts.tools_mut() {
            // Only run tools that are ready to execute. Resolved tools
            // (Completed/Rejected/Failed) are left alone.
            if tool.is_resolved() {
                continue;
            }

            let call = tool.effective_call().clone();
            tool.mark_running();

            match tools.call(call).await {
                Ok(response) => tool.complete(response),
                Err(err) => tool.fail(format!("{err:?}")),
            }

            executed = true;
        }

        Ok(executed)
    }

}
