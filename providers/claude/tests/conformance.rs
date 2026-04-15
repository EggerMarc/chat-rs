use chat_claude::ClaudeBuilder;
use chat_core::testing::conformance;
use chat_core::testing::{MockTransport, StaticToolDeclarations};
use chat_core::traits::CompletionProvider;
use chat_core::types::messages;

fn setup() -> (
    chat_claude::ClaudeClient<MockTransport>,
    chat_core::testing::TransportInspector,
) {
    let (transport, inspector) = MockTransport::new();
    let client = ClaudeBuilder::new()
        .with_model("claude-sonnet-4-20250514".to_string())
        .with_api_key("test-key".to_string())
        .with_thoughts(false)
        .with_transport(transport)
        .build();
    (client, inspector)
}

#[tokio::test]
async fn completion_roundtrip() {
    let (mut client, inspector) = setup();
    let mut msgs = messages::from_user(vec!["Hello"]);

    conformance::assert_completion_roundtrip(
        &mut client,
        &mut msgs,
        &inspector,
        include_bytes!("fixtures/text_response.json").to_vec(),
        "Hello! How can I help you today?",
    )
    .await;
}

#[tokio::test]
async fn tool_roundtrip() {
    let (mut client, inspector) = setup();
    let mut msgs = messages::from_user(vec!["What's the weather in SF?"]);
    let tools = StaticToolDeclarations(serde_json::json!([{
        "name": "get_weather",
        "description": "Get weather for a location",
        "input_schema": {
            "type": "object",
            "properties": {
                "location": { "type": "string" }
            },
            "required": ["location"]
        }
    }]));

    conformance::assert_tool_roundtrip(
        &mut client,
        &mut msgs,
        &tools,
        &inspector,
        include_bytes!("fixtures/tool_response.json").to_vec(),
        &["get_weather"],
    )
    .await;
}

#[tokio::test]
async fn rate_limit_maps_to_rate_limited() {
    let (mut client, inspector) = setup();
    let mut msgs = messages::from_user(vec!["Hello"]);

    conformance::assert_error_mapping(
        &mut client,
        &mut msgs,
        &inspector,
        429,
        b"rate limited".to_vec(),
        "RateLimited",
    )
    .await;
}

#[tokio::test]
async fn overloaded_maps_to_rate_limited() {
    let (mut client, inspector) = setup();
    let mut msgs = messages::from_user(vec!["Hello"]);

    conformance::assert_error_mapping(
        &mut client,
        &mut msgs,
        &inspector,
        529,
        b"overloaded".to_vec(),
        "RateLimited",
    )
    .await;
}

#[tokio::test]
async fn server_error_maps_to_provider() {
    let (mut client, inspector) = setup();
    let mut msgs = messages::from_user(vec!["Hello"]);

    conformance::assert_error_mapping(
        &mut client,
        &mut msgs,
        &inspector,
        500,
        b"internal server error".to_vec(),
        "Provider",
    )
    .await;
}

#[tokio::test]
async fn metadata_populated() {
    let (mut client, inspector) = setup();
    let mut msgs = messages::from_user(vec!["Hello"]);

    conformance::assert_metadata_populated(
        &mut client,
        &mut msgs,
        &inspector,
        include_bytes!("fixtures/text_response.json").to_vec(),
        |meta| {
            assert_eq!(meta.id.as_deref(), Some("msg_test_001"));
            assert_eq!(meta.model_slug.as_deref(), Some("claude-sonnet-4-20250514"));
            assert_eq!(meta.usage.input_tokens, 12);
            assert_eq!(meta.usage.output_tokens, 8);
            assert_eq!(meta.usage.total_tokens, 20);
        },
    )
    .await;
}

#[tokio::test]
async fn request_includes_api_key_header() {
    let (mut client, inspector) = setup();
    inspector.set_response(
        200,
        include_bytes!("fixtures/text_response.json").to_vec(),
    );
    let mut msgs = messages::from_user(vec!["Hello"]);

    client
        .complete(&mut msgs, None, None, None)
        .await
        .unwrap();

    let req = inspector.last_request().unwrap();
    let has_api_key = req
        .headers
        .iter()
        .any(|(k, v)| k == "x-api-key" && v == "test-key");
    assert!(has_api_key, "request should include x-api-key header");
    assert_eq!(req.path, "/v1/messages");
}
