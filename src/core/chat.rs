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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::content::{CompleteReasonEnum, RoleEnum};
    use crate::messages::parts::{PartEnum, Parts};
    use async_trait::async_trait;

    // Mock ChatProvider for testing
    struct MockProvider {
        responses: Vec<Content>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockProvider {
        fn new(responses: Vec<Content>) -> Self {
            Self {
                responses,
                call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            }
        }

        fn single_response(text: &str) -> Self {
            let content = Content {
                role: RoleEnum::Model,
                parts: Parts(vec![PartEnum::from_text(text.to_string())]),
                complete_reason: CompleteReasonEnum::Stop,
            };
            Self::new(vec![content])
        }
    }

    #[async_trait]
    impl ChatProvider for MockProvider {
        async fn complete(
            &mut self,
            _messages: &Messages,
            _tools: Option<&ToolCollection>,
            _options: Option<&ChatOptions>,
        ) -> Result<Content, ChatError> {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count;
            *count += 1;

            if idx < self.responses.len() {
                Ok(self.responses[idx].clone())
            } else {
                Err(ChatError::Provider("No more responses".to_string()))
            }
        }
    }

    #[test]
    fn test_chat_builder_new() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new();
        assert!(builder.model.is_none());
        assert!(builder.tools.is_none());
        assert!(builder.max_steps.is_none());
        assert!(builder.max_retries.is_none());
        assert!(builder.model_options.is_none());
    }

    #[test]
    fn test_chat_builder_default() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::default();
        assert!(builder.model.is_none());
    }

    #[test]
    fn test_chat_builder_with_model() {
        let provider = MockProvider::single_response("Test");
        let builder = ChatBuilder::new().with_model(provider);
        assert!(builder.model.is_some());
    }

    #[test]
    fn test_chat_builder_with_max_steps() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new()
            .with_max_steps(5);
        assert_eq!(builder.max_steps, Some(5));
    }

    #[test]
    fn test_chat_builder_with_max_retries() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new()
            .with_max_retries(3);
        assert_eq!(builder.max_retries, Some(3));
    }

    #[test]
    fn test_chat_builder_with_tools() {
        let tools = ToolCollection::new();
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new()
            .with_tools(tools.clone());
        assert!(builder.tools.is_some());
    }

    #[test]
    fn test_chat_builder_chaining() {
        let provider = MockProvider::single_response("Test");
        let tools = ToolCollection::new();
        
        let builder = ChatBuilder::new()
            .with_model(provider)
            .with_max_steps(10)
            .with_max_retries(2)
            .with_tools(tools);
        
        assert!(builder.model.is_some());
        assert_eq!(builder.max_steps, Some(10));
        assert_eq!(builder.max_retries, Some(2));
        assert!(builder.tools.is_some());
    }

    #[test]
    fn test_chat_builder_build() {
        let provider = MockProvider::single_response("Response");
        let chat = ChatBuilder::new()
            .with_model(provider)
            .with_max_steps(5)
            .build();
        
        assert_eq!(chat.max_steps, Some(5));
    }

    #[tokio::test]
    async fn test_chat_complete_simple() {
        let provider = MockProvider::single_response("Hello, user!");
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .build();
        
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Hi"]));
        
        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.role, RoleEnum::Model);
        assert_eq!(response.complete_reason, CompleteReasonEnum::Stop);
    }

    #[tokio::test]
    async fn test_chat_complete_with_multiple_steps() {
        let responses = vec![
            Content {
                role: RoleEnum::Model,
                parts: Parts(vec![PartEnum::from_reasoning("Thinking...".to_string())]),
                complete_reason: CompleteReasonEnum::None,
            },
            Content {
                role: RoleEnum::Model,
                parts: Parts(vec![PartEnum::from_text("Final answer".to_string())]),
                complete_reason: CompleteReasonEnum::Stop,
            },
        ];
        
        let provider = MockProvider::new(responses);
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .with_max_steps(5)
            .build();
        
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Question"]));
        
        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_complete_with_default_max_retries() {
        let provider = MockProvider::single_response("Response");
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .build();
        
        // Test that default max_retries is 1
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Test"]));
        
        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_complete_with_default_max_steps() {
        let provider = MockProvider::single_response("Response");
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .build();
        
        // Test that default max_steps is 1
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Test"]));
        
        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_complete_empty_messages() {
        let provider = MockProvider::single_response("Response");
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .build();
        
        let mut messages = Messages::default();
        
        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_complete_preserves_original_messages() {
        let provider = MockProvider::single_response("Response");
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .build();
        
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Original"]));
        let original_len = messages.len();
        
        let _result = chat.complete(&mut messages).await;
        
        // The complete method should not modify the input messages
        // (based on the code, it clones them for internal processing)
        assert_eq!(messages.len(), original_len);
    }

    #[test]
    fn test_chat_builder_multiple_max_steps_assignments() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new()
            .with_max_steps(5)
            .with_max_steps(10);
        
        assert_eq!(builder.max_steps, Some(10));
    }

    #[test]
    fn test_chat_builder_multiple_max_retries_assignments() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new()
            .with_max_retries(2)
            .with_max_retries(4);
        
        assert_eq!(builder.max_retries, Some(4));
    }

    #[tokio::test]
    async fn test_chat_with_system_and_user_messages() {
        let provider = MockProvider::single_response("I understand");
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .build();
        
        let mut messages = Messages::default();
        messages.push(content::from_system(vec!["Be helpful"]));
        messages.push(content::from_user(vec!["Help me"]));
        
        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_complete_reasoning_only_response() {
        let response = Content {
            role: RoleEnum::Model,
            parts: Parts(vec![PartEnum::from_reasoning("Analyzing...".to_string())]),
            complete_reason: CompleteReasonEnum::None,
        };
        
        let provider = MockProvider::new(vec![response]);
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .with_max_steps(1)
            .build();
        
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Test"]));
        
        // With max_steps=1, it should handle reasoning-only response
        let _result = chat.complete(&mut messages).await;
    }

    #[test]
    fn test_chat_builder_zero_max_steps() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new()
            .with_max_steps(0);
        assert_eq!(builder.max_steps, Some(0));
    }

    #[test]
    fn test_chat_builder_zero_max_retries() {
        let builder: ChatBuilder<MockProvider> = ChatBuilder::new()
            .with_max_retries(0);
        assert_eq!(builder.max_retries, Some(0));
    }

    #[test]
    fn test_chat_struct_creation() {
        let provider = MockProvider::single_response("Test");
        let tools = ToolCollection::new();
        let options = ChatOptions::default();
        
        let chat = Chat {
            model: provider,
            tools: Some(tools),
            max_steps: Some(5),
            max_retries: Some(2),
            model_options: Some(options),
        };
        
        assert_eq!(chat.max_steps, Some(5));
        assert_eq!(chat.max_retries, Some(2));
        assert!(chat.tools.is_some());
        assert!(chat.model_options.is_some());
    }

    #[tokio::test]
    async fn test_chat_complete_with_chat_options() {
        let provider = MockProvider::single_response("Response");
        let options = ChatOptions::default();
        
        let mut chat = ChatBuilder::new()
            .with_model(provider)
            .with_model_options(options)
            .build();
        
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Test"]));
        
        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_chat_builder_full_configuration() {
        let provider = MockProvider::single_response("Test");
        let tools = ToolCollection::new();
        let options = ChatOptions::default();
        
        let chat = ChatBuilder::new()
            .with_model(provider)
            .with_tools(tools)
            .with_max_steps(10)
            .with_max_retries(3)
            .with_model_options(options)
            .build();
        
        assert_eq!(chat.max_steps, Some(10));
        assert_eq!(chat.max_retries, Some(3));
        assert!(chat.tools.is_some());
        assert!(chat.model_options.is_some());
    }
}
