use async_recursion::async_recursion;
use tools_rs::ToolCollection;

use crate::core::{
    lib::{ChatOptions, ChatProvider},
    messages::{Messages, content::Content, parts::PartEnum},
};

#[derive(Default)]
pub struct Chat<C: ChatProvider, Shape: serde::de::DeserializeOwned + Clone + Default + Sync> {
    model: C,
    model_options: ChatOptions<Shape>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<C: ChatProvider, Shape: serde::de::DeserializeOwned + Clone + Default + Sync> Chat<C, Shape> {
    #[async_recursion]
    async fn complete(
        &self,
        messages: &mut Messages,
    ) -> Result<Content, Box<dyn std::error::Error + Send + Sync>> {
        // Let's first do it without structured outputs
        let mut completion = self
            .model
            .complete(messages, self.tools.as_ref(), self.model_options.clone())
            .await?;

        if let Some(tools) = &self.tools {
            let completion_parts = completion.parts.clone();
            let function_calls = completion_parts
                .function_calls()
                .filter(|fc| {
                    completion_parts
                        .function_response(
                            fc.id
                                .clone()
                                .expect("Cannot respond to a function without id"),
                        )
                        .is_none()
                })
                .collect::<Vec<_>>();

            for fc in function_calls {
                let res = tools.call(fc.clone()).await;
                if let Ok(fr) = res {
                    completion.parts.push(PartEnum::FunctionResponse(fr));
                }
            }
        }

        if completion.parts.text_response().is_some() {
            Ok(completion)
        } else {
            // This merges on same RoleEnum
            messages.push(completion);
            return self.complete(messages).await;
        }
    }
}

pub struct ChatBuilder<C: ChatProvider, Shape: serde::de::DeserializeOwned + Clone + Default> {
    model: Option<C>,
    model_options: Option<ChatOptions<Shape>>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<C: ChatProvider, Shape: serde::de::DeserializeOwned + Clone + Default + Sync>
    ChatBuilder<C, Shape>
{
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

    pub fn build(self) -> Chat<C, Shape> {
        Chat {
            model: self.model.expect("Need to set a model"),
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            tools: self.tools,
            model_options: self.model_options.unwrap_or_else(ChatOptions::default),
        }
    }
}
