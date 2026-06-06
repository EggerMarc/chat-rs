//! Bridge reply JSON → `ChatResponse` (or a mapped `ChatFailure`).

use chat_core::error::{ChatError, ChatFailure};
use chat_core::types::messages::content::{CompleteReasonEnum, Content, RoleEnum};
use chat_core::types::messages::parts::{PartEnum, Parts};
use chat_core::types::messages::text::Text;
use chat_core::types::metadata::Metadata;
use chat_core::types::response::ChatResponse;

use super::{CompleteReply, ErrorReply};

pub(crate) fn into_core(model_slug: &str, reply_json: &str) -> Result<ChatResponse, ChatFailure> {
    if let Ok(err) = serde_json::from_str::<ErrorReply>(reply_json) {
        return Err(map_error(err));
    }

    let reply: CompleteReply = serde_json::from_str(reply_json).map_err(|e| {
        ChatFailure::from_err(ChatError::InvalidResponse(format!(
            "malformed bridge reply ({e}): {reply_json}"
        )))
    })?;

    let complete_reason = map_finish(&reply.finish);

    Ok(ChatResponse {
        metadata: Some(Metadata {
            model_slug: Some(model_slug.to_owned()),
            ..Default::default()
        }),
        content: Content {
            role: RoleEnum::Model,
            parts: Parts(vec![PartEnum::Text(Text::new(reply.text))]),
            complete_reason,
        },
    })
}

pub(crate) fn map_finish(finish: &str) -> CompleteReasonEnum {
    match finish {
        "stop" => CompleteReasonEnum::Stop,
        "max_tokens" => CompleteReasonEnum::MaxTokens,
        other => CompleteReasonEnum::Other(other.to_owned()),
    }
}

/// The bridge's error kinds, mapped onto chat-rs error semantics. All are
/// non-retryable `Provider` errors except `internal`, which is `Other`.
pub(crate) fn error_to_chat(error: super::ErrorBody) -> ChatError {
    match error.kind.as_str() {
        "internal" => ChatError::Other(format!("applefm bridge: {}", error.message)),
        kind => ChatError::Provider(format!("applefm {kind}: {}", error.message)),
    }
}

fn map_error(reply: ErrorReply) -> ChatFailure {
    let ErrorReply { error } = reply;
    ChatFailure::from_err(error_to_chat(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_completion() {
        let resp = into_core("apple-on-device", r#"{"text":"hi there","finish":"stop"}"#).unwrap();
        assert_eq!(
            resp.content.parts.text_response().map(|t| t.as_str()),
            Some("hi there")
        );
        assert!(matches!(
            resp.content.complete_reason,
            CompleteReasonEnum::Stop
        ));
    }

    #[test]
    fn maps_error_kinds() {
        let err = into_core(
            "apple-on-device",
            r#"{"error":{"kind":"unavailable","message":"not enabled"}}"#,
        )
        .unwrap_err();
        assert!(matches!(err.err, ChatError::Provider(_)));
        assert!(err.err.to_string().contains("unavailable"));
    }
}
