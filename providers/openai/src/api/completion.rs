use crate::api::types::request::OpenAIResponsesRequest;
use crate::api::types::response::ResponsesApiResponse;
use crate::client::OpenAIClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use tools_rs::ToolCollection;

#[async_trait::async_trait]
impl CompletionProvider for OpenAIClient {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let url = format!("{}/responses", self.base_url);

        let previous_response_id = if self.use_previous_response_id {
            self.last_response_id.clone()
        } else {
            None
        };

        let request_body = OpenAIResponsesRequest::from_core(
            crate::api::types::request::ResponsesRequestConfig {
                model_name: &self.model_name,
                messages,
                custom_tools: tools,
                native_tools: self.native_tools.as_slice(),
                reasoning_effort: self.reasoning_effort.clone(),
                options,
                output_shape: structured_output,
                previous_response_id,
                store: self.store,
            },
        )
        .map_err(ChatFailure::from_err)?;

        let res = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::Network(e.to_string())))?;
        let res = res
            .error_for_status()
            .map_err(|e| ChatFailure::from_err(ChatError::Provider(e.to_string())))?;

        let oai_data: ResponsesApiResponse = res
            .json()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        let (response, response_id) = oai_data
            .into_core_chat_response()
            .map_err(ChatFailure::from_err)?;

        if self.use_previous_response_id
            && let Some(id) = response_id
        {
            self.last_response_id = Some(id);
        }

        Ok(response)
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::request::ResponsesRequestConfig;

    #[test]
    fn test_from_core_without_previous_response_id() {
        let messages = chat_core::types::messages::from_user(vec!["hello"]);
        let req = OpenAIResponsesRequest::from_core(ResponsesRequestConfig {
            model_name: "gpt-4o",
            messages: &messages,
            custom_tools: None,
            native_tools: &[],
            reasoning_effort: None,
            options: None,
            output_shape: None,
            previous_response_id: None,
            store: None,
        })
        .unwrap();

        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("previous_response_id").is_none());
    }

    #[test]
    fn test_from_core_with_previous_response_id() {
        let messages = chat_core::types::messages::from_user(vec!["hello"]);
        let req = OpenAIResponsesRequest::from_core(ResponsesRequestConfig {
            model_name: "gpt-4o",
            messages: &messages,
            custom_tools: None,
            native_tools: &[],
            reasoning_effort: None,
            options: None,
            output_shape: None,
            previous_response_id: Some("resp_abc".to_string()),
            store: None,
        })
        .unwrap();

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["previous_response_id"], "resp_abc");
    }
}
