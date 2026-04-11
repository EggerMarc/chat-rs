use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};

use crate::{
    chat::{Chat, state::Unstructured},
    error::ChatFailure,
    traits::StreamProvider,
    types::{
        messages::Messages,
        metadata::Metadata,
        response::{ChatResponse, StreamEvent},
    },
};

impl<CP: StreamProvider> Chat<CP, Unstructured> {
    /// Streaming chat loop with HITL support.
    ///
    /// Yields each token/chunk as `StreamEvent::TextChunk` / similar. When
    /// a tool strategy pauses execution (for example, `RequireApproval`),
    /// the stream yields `StreamEvent::Paused(PauseReason)` and then
    /// terminates. The caller resolves pending tools on `messages` —
    /// typically via `Messages::find_tool_mut` — and calls `stream()`
    /// again to continue. On re-entry, a pre-step executes any
    /// newly-approved tools, emits `ToolResult` events for them, and
    /// then falls through into the next provider turn.
    pub async fn stream<'a>(
        &'a mut self,
        messages: &'a mut Messages,
    ) -> Result<BoxStream<'a, Result<StreamEvent, ChatFailure>>, ChatFailure> {
        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }

        let stream = try_stream! {
            let max_steps = self.max_steps.unwrap_or(1);
            let mut last_metadata: Option<Metadata> = None;

            for _ in 0..max_steps {
                // Pre-step: execute any tools already resolved to
                // Approved on the last Content (typically from a
                // prior pause that the caller just resolved). Emit
                // ToolResult events for completed tools. Yield Paused
                // if the pre-step itself produced a pause (can happen
                // if the caller left some tools still Pending).
                if let Some(last) = messages.0.last_mut() {
                    let pass = self
                        .tool_call(last)
                        .await
                        .map_err(|err| ChatFailure {
                            err,
                            metadata: last_metadata.clone(),
                        })?;

                    if pass.executed
                        && let Some(last) = messages.0.last()
                    {
                        for tool in last.parts.tools() {
                            if let Some(fr) = tool.response() {
                                yield StreamEvent::ToolResult(fr.clone());
                            }
                        }
                    }

                    if let Some(reason) = pass.pause {
                        yield StreamEvent::Paused(reason);
                        return;
                    }
                }

                let decls =
                    crate::chat::tool_declarations_from(&self.scoped_collections);
                let decls_dyn = decls
                    .as_ref()
                    .map(|d| d as &dyn crate::types::tools::ToolDeclarations);
                let mut provider_stream = self
                    .model
                    .stream(messages, decls_dyn, self.model_options.as_ref())
                    .await
                    .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?;

                let mut final_response: Option<ChatResponse> = None;

                while let Some(event_result) = provider_stream.next().await {
                    match event_result {
                        Ok(StreamEvent::Done(response)) => {
                            final_response = Some(response);
                        }
                        Ok(event) => {
                            yield event;
                        }
                        Err(err) => {
                            Err(ChatFailure { err, metadata: last_metadata.clone() })?;
                        }
                    }
                }

                if let Some(response) = final_response {
                    self.model.on_stream_done(&response);

                    if let Some(metadata) = response.metadata.clone() {
                        match &mut last_metadata {
                            Some(existing) => { existing.extend(&metadata); },
                            None => { last_metadata = Some(metadata); },
                        }
                    }

                    messages.push(response.content.clone());

                    // Post-step: apply strategy to any tools the model
                    // emitted this turn. Execute those that say Execute;
                    // pause on anything that needs approval/deferral.
                    let pass = match messages.0.last_mut() {
                        Some(last) => self.tool_call(last).await
                            .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?,
                        None => crate::chat::ToolCallPass::default(),
                    };

                    if pass.executed
                        && let Some(last) = messages.0.last()
                    {
                        for tool in last.parts.tools() {
                            if let Some(fr) = tool.response() {
                                yield StreamEvent::ToolResult(fr.clone());
                            }
                        }
                    }

                    if let Some(reason) = pass.pause {
                        yield StreamEvent::Paused(reason);
                        return;
                    }

                    if pass.executed {
                        // Tools ran; need another provider turn so the
                        // model can react to the results.
                        continue;
                    }

                    if let Some(strategy) = self.after_strategy.as_mut() {
                        strategy(messages, last_metadata.as_ref()).await;
                    }
                    yield StreamEvent::Done(response);
                    break;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
