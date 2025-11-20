use tools_rs::ToolCollection;

use crate::core::{
    lib::{ChatError, ChatOptions, ChatProvider},
    messages::{
        Messages,
        content::{CompleteReasonEnum, Content},
        parts::{PartEnum, Parts},
    },
};

#[derive(Default)]
pub struct Chat<CP: ChatProvider> {
    model: CP,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<CP: ChatProvider> Chat<CP> {
    pub async fn complete(&mut self, messages: &mut Messages) -> Result<Content, ChatError> {
        let max_retries = self.max_retries.unwrap_or(1);
        for _ in 0..max_retries {
            let retry_messages = messages.clone();
            return match self.call_loop(&retry_messages).await {
                Ok(content) => Ok(content),
                Err(_) => continue,
            };
        }
        Err(ChatError::RateLimited)
    }

    async fn tool_call(&self, content: &Content) -> Result<Parts, ChatError> {
        let mut frs: Parts = Parts::default();
        for fc in content.parts.function_calls() {
            frs.push(PartEnum::from_function_response(
                self.tools
                    .clone()
                    .ok_or(ChatError::InvalidResponse(
                        "Attempted to call tool but no tool collection has been set.".to_string(),
                    ))?
                    .call(fc.clone())
                    .await
                    .map_err(|_err| ChatError::InvalidResponse("Tools error: {}".to_string()))?,
            ));
        }
        Ok(frs)
    }

    async fn call_loop(&mut self, messages: &Messages) -> Result<Content, ChatError> {
        let mut inner_messages = messages.clone();
        for _ in 0..self.max_steps.unwrap_or(1) {
            let mut response = self
                .model
                .complete(
                    &inner_messages,
                    self.tools.as_ref(),
                    self.model_options.as_ref(),
                )
                .await?;
            if let Ok(frs) = self.tool_call(&response).await
                && frs.length() > 0
            {
                response.parts.extend(frs);
            }

            match response.parts.last() {
                Some(res) => match res {
                    PartEnum::Text(_text) => return Ok(response),
                    PartEnum::Reasoning(reasoning) => {
                        response
                            .parts
                            .push(PartEnum::from_reasoning(reasoning.to_owned()));
                    }
                    PartEnum::Structured(_) => {
                        return Err(ChatError::Other(
                            "Structured output not yet implemented".to_string(),
                        ));
                    }
                    _ => {}
                },
                None => {
                    return Err(ChatError::InvalidResponse(
                        "Response did not generate any parts".to_string(),
                    ));
                }
            };

            inner_messages.push(response.clone());
            println!(
                "Inner Messages: {:#?}\n Response (should match last inner message): {:#?}",
                inner_messages, response
            );
            /*
                            return match response.complete_reason {
                                CompleteReasonEnum::Stop => Ok(response),
                                CompleteReasonEnum::MaxTokens => Err(ChatError::RateLimited),
                                CompleteReasonEnum::Recitation => Err(ChatError::Provider(
                                    "Content response was recited".to_string(),
                                )),
                                CompleteReasonEnum::ContentFilter => Err(ChatError::Provider(
                                    "Content response was filtered".to_string(),
                                )),
                                CompleteReasonEnum::ToolCall => {
                                    // Gemini doesn't have this.
                                    response.parts.extend(self.tool_call(&response).await?);
                                    continue;
                                }
                                CompleteReasonEnum::None => {
                                    // Default implementation for completion when no stopping reason is provided
                                }
                            };
            */
        }
        Err(ChatError::RateLimited)
    }
}

pub struct ChatBuilder<CP: ChatProvider> {
    model: Option<CP>,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<CP: ChatProvider> ChatBuilder<CP> {
    pub fn new() -> Self {
        ChatBuilder {
            model: None,
            max_steps: None,
            max_retries: None,
            tools: None,
            model_options: None,
        }
    }

    pub fn with_max_steps(mut self, max_steps: i16) -> Self {
        self.max_steps = Some(max_steps);
        self
    }

    pub fn with_max_retries(mut self, max_retries: i16) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    pub fn with_tools(mut self, tools: ToolCollection) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_model(mut self, model: CP) -> Self {
        self.model = Some(model);
        self
    }

    pub fn build(self) -> Chat<CP> {
        Chat {
            model: self.model.expect("Need to set a model"),
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            tools: self.tools,
            model_options: self.model_options,
        }
    }
}

impl<CP: ChatProvider> Default for ChatBuilder<CP> {
    fn default() -> Self {
        ChatBuilder::new()
    }
}
