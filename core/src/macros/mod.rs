#[macro_export]
macro_rules! retry_strategy {
    (|$ctx:ident| $body:block) => {
        Box::new(move |_messages: &mut $crate::types::messages::Messages, _metadata: Option<&$crate::types::metadata::Metadata>, $ctx: $crate::types::callback::CallbackRetryContext| {
            Box::pin(async move $body)
        })
    };
}
