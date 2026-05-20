use chat_core::types::messages::content::{CompleteReasonEnum, Content, RoleEnum};
use chat_core::types::messages::parts::{PartEnum, Parts};
use chat_core::types::messages::text::Text;
use chat_core::types::metadata::Metadata;
use chat_core::types::metadata::usage::Usage as CoreUsage;
use chat_core::types::response::ChatResponse;
use mistralrs::{ChatCompletionResponse, Usage as MUsage};

use crate::api::types::error::invalid_response;
use chat_core::error::ChatFailure;

/// Convert mistral.rs's [`ChatCompletionResponse`] to chat-rs's [`ChatResponse`].
pub fn into_core(
    model_id: &str,
    resp: ChatCompletionResponse,
) -> Result<ChatResponse, ChatFailure> {
    let choice = resp
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| invalid_response("mistral.rs response has no choices"))?;

    let text = choice.message.content.unwrap_or_default();
    let reason_str = choice.finish_reason;
    let complete_reason = map_finish_reason(if reason_str.is_empty() {
        None
    } else {
        Some(reason_str.as_str())
    });

    let parts = Parts(vec![PartEnum::Text(Text::new(text))]);
    let content = Content {
        role: RoleEnum::Model,
        parts,
        complete_reason,
    };

    let metadata = Metadata {
        id: Some(resp.id),
        model_slug: Some(model_id.to_string()),
        usage: usage_from_m(resp.usage),
        ..Default::default()
    };

    Ok(ChatResponse {
        metadata: Some(metadata),
        content,
    })
}

pub(crate) fn map_finish_reason(reason: Option<&str>) -> CompleteReasonEnum {
    match reason {
        Some("stop") => CompleteReasonEnum::Stop,
        Some("length") => CompleteReasonEnum::MaxTokens,
        Some("tool_calls") => CompleteReasonEnum::ToolCall,
        Some(other) => CompleteReasonEnum::Other(other.to_string()),
        None => CompleteReasonEnum::None,
    }
}

pub(crate) fn usage_from_m(u: MUsage) -> CoreUsage {
    CoreUsage {
        input_tokens: u.prompt_tokens as usize,
        output_tokens: u.completion_tokens as usize,
        total_tokens: u.total_tokens as usize,
    }
}
