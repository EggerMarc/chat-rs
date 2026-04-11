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

                    let executed = match messages.0.last_mut() {
                        Some(last) => self.tool_call(last).await.unwrap_or(false),
                        None => false,
                    };

                    if executed {
                        if let Some(last) = messages.0.last() {
                            for tool in last.parts.tools() {
                                if let Some(fr) = tool.response() {
                                    yield StreamEvent::ToolResult(fr.clone());
                                }
                            }
                        }
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
