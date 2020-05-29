// This Lambda function shows how you might run async code in your handler, even though minlambda
// lacks any first-class support for async code.

use futures_util::future::TryFutureExt;
use serde_derive::Serialize;

#[derive(Debug, Serialize)]
struct HandlerResponse {
    body: String,
}

fn main() {
    let mut runtime = tokio::runtime::Runtime::new().unwrap();
    minlambda::run(|_: serde::de::IgnoredAny| {
        runtime.block_on(async {
            reqwest::get("https://www.example.com/")
                .and_then(|response| response.text())
                .await
                .map(|body| HandlerResponse { body })
        })
    })
}
