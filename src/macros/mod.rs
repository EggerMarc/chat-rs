#[macro_export]
macro_rules! retry_strategy {
    (|$ctx:ident| $body:block) => {
        Box::new(move |$ctx| {
            Box::pin(async move $body)
        })
    };
}
