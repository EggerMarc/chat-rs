use crate::error::ChatError;
use crate::traits::CompletionProvider;
use crate::types::messages::content::{CompleteReasonEnum, RoleEnum};
use crate::types::messages::Messages;
use crate::types::messages::tool::ToolStatus;
use crate::types::metadata::Metadata;
use crate::types::tools::ToolDeclarations;

use super::TransportInspector;

/// Verify that a provider correctly maps a canned API response into a `ChatResponse`
/// with the expected text content.
///
/// 1. Queues the canned response bytes on the mock transport.
/// 2. Calls `provider.complete()`.
/// 3. Asserts the response has `RoleEnum::Model`, `CompleteReasonEnum::Stop`,
///    and its first text part matches `expected_text`.
/// 4. Asserts exactly one request was sent.
pub async fn assert_completion_roundtrip(
    provider: &mut impl CompletionProvider,
    messages: &mut Messages,
    inspector: &TransportInspector,
    canned_response: impl Into<Vec<u8>>,
    expected_text: &str,
) {
    inspector.set_response(200, canned_response);

    let response = provider
        .complete(messages, None, None, None)
        .await
        .expect("provider.complete() should succeed with canned 200 response");

    assert_eq!(
        response.content.role,
        RoleEnum::Model,
        "response role should be Model"
    );
    assert_eq!(
        response.content.complete_reason,
        CompleteReasonEnum::Stop,
        "complete_reason should be Stop for a text-only response"
    );

    let text = response
        .content
        .parts
        .text_response()
        .expect("response should contain a text part");
    assert_eq!(
        text.as_str(),
        expected_text,
        "response text should match expected"
    );

    assert_eq!(
        inspector.request_count(),
        1,
        "provider should have sent exactly one request"
    );
}

/// Verify that a provider correctly parses tool calls from a canned response.
///
/// 1. Queues the canned response bytes.
/// 2. Calls `provider.complete()` with the given tool declarations.
/// 3. Asserts the response contains Tool parts with the expected names,
///    all in `Pending` status, and `complete_reason` is `ToolCall`.
pub async fn assert_tool_roundtrip(
    provider: &mut impl CompletionProvider,
    messages: &mut Messages,
    tools: &dyn ToolDeclarations,
    inspector: &TransportInspector,
    canned_response: impl Into<Vec<u8>>,
    expected_tool_names: &[&str],
) {
    inspector.set_response(200, canned_response);

    let response = provider
        .complete(messages, Some(tools), None, None)
        .await
        .expect("provider.complete() should succeed with canned tool response");

    let tool_parts: Vec<_> = response.content.parts.tools().collect();
    assert_eq!(
        tool_parts.len(),
        expected_tool_names.len(),
        "number of tool calls should match expected (got {:?})",
        tool_parts
            .iter()
            .map(|t| &t.call.name)
            .collect::<Vec<_>>()
    );

    for (tool, expected_name) in tool_parts.iter().zip(expected_tool_names) {
        assert_eq!(
            &tool.call.name, expected_name,
            "tool call name should match"
        );
        assert!(
            matches!(tool.status, ToolStatus::Pending),
            "tool should be in Pending status, got {:?}",
            tool.status
        );
    }
}

/// Verify that a provider maps a given HTTP status code to the expected `ChatError` variant.
///
/// 1. Queues a response with the given status and body.
/// 2. Calls `provider.complete()`.
/// 3. Asserts the result is an error with the expected variant.
///
/// `expected_variant` is one of: `"Network"`, `"Provider"`, `"RateLimited"`,
/// `"MaxStepsExceeded"`, `"InvalidResponse"`, `"Callback"`, `"Other"`.
pub async fn assert_error_mapping(
    provider: &mut impl CompletionProvider,
    messages: &mut Messages,
    inspector: &TransportInspector,
    status: u16,
    body: impl Into<Vec<u8>>,
    expected_variant: &str,
) {
    inspector.set_response(status, body);

    let result = provider.complete(messages, None, None, None).await;

    let failure = result.expect_err(&format!(
        "provider.complete() should fail with status {}",
        status
    ));

    let actual_variant = error_variant_name(&failure.err);
    assert_eq!(
        actual_variant, expected_variant,
        "expected ChatError::{expected_variant}, got ChatError::{actual_variant}: {}",
        failure.err
    );
}

/// Verify that metadata is populated from a canned response.
///
/// 1. Queues the canned response bytes.
/// 2. Calls `provider.complete()`.
/// 3. Passes the metadata to the provided `check` closure for flexible assertions.
pub async fn assert_metadata_populated(
    provider: &mut impl CompletionProvider,
    messages: &mut Messages,
    inspector: &TransportInspector,
    canned_response: impl Into<Vec<u8>>,
    check: impl FnOnce(&Metadata),
) {
    inspector.set_response(200, canned_response);

    let response = provider
        .complete(messages, None, None, None)
        .await
        .expect("provider.complete() should succeed with canned response");

    let metadata = response
        .metadata
        .as_ref()
        .expect("response should contain metadata");

    check(metadata);
}

fn error_variant_name(err: &ChatError) -> &'static str {
    match err {
        ChatError::Network(_) => "Network",
        ChatError::Provider(_) => "Provider",
        ChatError::RateLimited => "RateLimited",
        ChatError::MaxStepsExceeded => "MaxStepsExceeded",
        ChatError::InvalidResponse(_) => "InvalidResponse",
        ChatError::Callback(_) => "Callback",
        ChatError::Other(_) => "Other",
    }
}
