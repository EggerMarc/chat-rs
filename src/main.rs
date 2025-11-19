mod core;
mod providers;
use core::messages;
use core::messages::parts::Parts;

use serde_json::json;
use tools_rs::{FunctionCall, FunctionResponse};

use crate::core::chat::ChatBuilder;
use crate::core::messages::Messages;
use crate::core::messages::content::{CompleteReasonEnum, Content, RoleEnum};
use crate::core::messages::parts::PartEnum;

fn main() {
    let prompt = messages::from_user(vec!["Hello there mom!"]);
    let sys_prompt = messages::from_system(vec![
        "Answer the user with clarity at all times. Be a good boy.",
    ]);
    let mut messages = Messages(vec![sys_prompt, prompt]);

    // Dummy responses
    let reasoning_part =
        PartEnum::from_reasoning("I think eating an icecream would be a great idea!".to_string());
    let fc = FunctionCall::new(
        "buy_icecream".to_string(),
        json!({
        "flavor": "Vanilla",
        "quantity": 10
        }),
    );
    let fr = FunctionResponse {
        id: fc.id.clone(),
        name: "buy_icecream".to_string(),
        result: json!({
            "status": 200,
            "response": "bought it"
        }),
    };

    let fc_part = PartEnum::from_function_call(fc.clone());
    let fr_part = PartEnum::from_function_response(fr);
    let response_part = PartEnum::from_text("I jus ate some icecream!".to_string());

    let ai_content = Content {
        role: RoleEnum::Model,
        parts: Parts(vec![reasoning_part, fc_part, fr_part])
            .push(response_part)
            .to_owned(),
        complete_reason: CompleteReasonEnum::Stop,
    };

    messages.push(ai_content.clone());
    println!("{:#?}", messages);
    println!(
        "Function #: {:#?}\n\tReturned: {:?}",
        fc.id.clone(),
        ai_content
            .parts
            .function_response(fc.id.expect("didn't set a function id")),
    );

    let mut model = ChatBuilder::new()
        //.with_model(providers::gemini::Client)
        .with_model(providers::gemini::GeminiClient::new("flash-1.5"))
        .with_max_steps(5)
        .with_max_retries(2)
        .build();

    model.complete(&mut messages);
}
