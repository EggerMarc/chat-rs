use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};

use crate::{
    chat::{Chat, state::Unstructured},
    error::ChatFailure,
    traits::StreamProvider,
    types::{
        messages::{Messages, content::Content, parts::PartEnum},
        metadata::Metadata,
        response::{ChatResponse, StreamEvent}, // Ensure StreamEvent is imported
    },
};

impl<CP: StreamProvider> Chat<CP, Unstructured> {
    pub async fn stream<'a>(
        &'a mut self,
        messages: &'a mut Messages,
        // 1. CHANGED: Now returns a Stream of StreamEvent, not String!
    ) -> Result<BoxStream<'a, Result<StreamEvent, ChatFailure>>, ChatFailure> {
        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }

        let stream = try_stream! {
            let max_steps = self.max_steps.unwrap_or(1);
            let mut last_metadata: Option<Metadata> = None;

            for _ in 0..max_steps {
                let mut provider_stream = self
                    .model
                    .stream(messages, self.tools.as_ref(), self.model_options.as_ref())
                    .await
                    .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?;

                let mut final_response: Option<ChatResponse> = None;

                while let Some(event_result) = provider_stream.next().await {
                    match event_result {
                        Ok(StreamEvent::Done(response)) => {
                            final_response = Some(response);
                        }
                        Ok(event) => {
                            // 2. SIMPLIFIED: Just yield the event transparently to the user!
                            yield event;
                        }
                        Err(err) => {
                            Err(ChatFailure { err, metadata: last_metadata.clone() })?;
                        }
                    }
                }

                if let Some(response) = final_response {
                    if let Some(metadata) = response.metadata.clone() {
                        match &mut last_metadata {
                            Some(existing) => { existing.extend(&metadata); },
                            None => { last_metadata = Some(metadata); },
                        }
                    }

                    messages.push(response.content.clone());

                    // 3. DE-DUPLICATED: Only one tool_call block using stable Rust
                    if let Ok(frs) = self.tool_call(&response.content).await &&!frs.is_empty() {
                            let mut tool_message = Content::default();

                            for part in frs.0 {
                                if let PartEnum::FunctionResponse(ref fr) = part {
                                    // Yield the tool result so the UI knows we finished executing!
                                    yield StreamEvent::ToolResult(fr.clone());
                                }
                                tool_message.parts.push(part);
                            }

                            messages.push(tool_message);
                            continue; // Loop back to the model with the tool output
                    }

                    if let Some(strategy) = self.after_strategy.as_mut() {
                        strategy(messages, last_metadata.as_ref()).await;
                    }

                    // Yield the final Done event to close out the stream cleanly
                    yield StreamEvent::Done(response);
                    break;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
