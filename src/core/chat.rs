use async_recursion::async_recursion;
use tools_rs::{FunctionResponse, ToolCollection};

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
    model_options: ChatOptions,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<CP: ChatProvider> Chat<CP> {
    #[async_recursion]
    pub async fn complete(
        &self,
        messages: &mut Messages,
    ) -> Result<Content, Box<dyn std::error::Error + Send + Sync>> {
        let max_retries = self.max_retries.unwrap_or(0);
        let max_steps = self.max_steps.unwrap_or(0);
        Err(ChatError::RateLimited)

}

pub struct ChatBuilder<C: ChatProvider> {
    model: Option<C>,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<C: ChatProvider> ChatBuilder<C> {
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

    pub fn with_model(mut self, model: C) -> Self {
        self.model = Some(model);
        self
    }

    pub fn build(self) -> Chat<C> {
        Chat {
            model: self.model.expect("Need to set a model"),
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            tools: self.tools,
            model_options: self.model_options.unwrap_or_else(ChatOptions::default),
        }
    }
}

async fn call_loop<CP: ChatProvider>(
    chat: &mut CP,
    messages: &Messages,
    max_steps: &Option<u16>,
    tools: Option<&ToolCollection>,
    options: Option<&ChatOptions>,
) -> Result<Content, ChatError> {
    for _ in 0..max_steps.unwrap_or(0) {
        let mut response = chat.complete(messages, tools, options).await?;

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
                response.parts.extend(
                    tool_call(
                        &response,
                        tools.ok_or_else(|| {
                            ChatError::InvalidResponse(
                                "Calling tool when there is none set".to_string(),
                            )
                        })?,
                    )
                    .await?,
                );
                continue;
            }
            CompleteReasonEnum::None => {
                // Default implementation for completion when no stopping reason is provided
                let fcs = tool_call(
                    &response,
                    tools.ok_or_else(|| {
                        ChatError::InvalidResponse(
                            "Calling tool when there is none set".to_string(),
                        )
                    })?,
                )
                .await?;

                if fcs.length() > 0 {
                    response.parts.extend(fcs);
                    continue;
                } else {
                    match response.parts.last() {
                        Some(res) => match res {
                            PartEnum::Text(_text) => return Ok(response),
                            PartEnum::Reasoning(reasoning) => {
                                response
                                    .parts
                                    .push(PartEnum::from_reasoning(reasoning.to_owned()));
                                continue;
                            }
                            PartEnum::FunctionResponse(fr) => {
                                response
                                    .parts
                                    .push(PartEnum::from_function_response(fr.clone()));
                                continue;
                            }
                            PartEnum::FunctionCall(fc) => {


                            },
                            PartEnum::Structured(_) => ChatError::Other(
                                "Structured output not yet implemented".to_string(),
                            ),
                        },
                        None => {
                            return Err(ChatError::InvalidResponse(
                                "Response did not generate any parts".to_string(),
                            ));
                        }
                    };
                }

                continue;
            }
        };
    }
    Err(ChatError::RateLimited)
}

async fn tool_call(content: &Content, tools: &ToolCollection) -> Result<Parts, ChatError> {
    let mut frs: Parts = Parts::default();
    for fc in content.parts.function_calls() {
        frs.push(PartEnum::from_function_response(
            tools
                .call(fc.clone())
                .await
                .map_err(|err| ChatError::InvalidResponse(err.to_string()))?,
        ));
    }
    Ok(frs)
}
