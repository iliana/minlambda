#![warn(clippy::pedantic)]

mod http;

use serde::{de::DeserializeOwned, Serialize};
use std::net::SocketAddr;

/// Retrieves invocation events, calls your handler, and sends back response data within the Lambda
/// execution environment.
///
/// # Panics
///
/// This function panics on two fatal error conditions:
///
/// * Failing to parse the `AWS_LAMBDA_RUNTIME_API` environment variable as a [`SocketAddr`].
/// * Failing to report an error to the runtime interface.
pub fn run<F, D, S, E>(handler: F) -> !
where
    F: FnMut(D) -> Result<S, E>,
    D: DeserializeOwned,
    S: Serialize,
    E: std::error::Error + 'static,
{
    let addr: SocketAddr = std::env::var("AWS_LAMBDA_RUNTIME_API")
        .expect("could not get $AWS_LAMBDA_RUNTIME_API")
        .parse()
        .expect("could not parse $AWS_LAMBDA_RUNTIME_API as SocketAddr");
    let mut handler = handler;

    loop {
        if let Err(inner_err) = run_inner(addr, &mut handler) {
            if let Err(init_err) = http::post_error(
                addr,
                "init/error",
                "minlambda::Error",
                &inner_err.to_string(),
            ) {
                panic!(
                    "failed to report initialization error: {:?}\ncaused by: {:?}",
                    init_err, inner_err
                );
            }
        }
    }
}

/// The same as [`run`] but for handlers that don't return [`Result`].
pub fn run_ok<F, D, S>(handler: F) -> !
where
    F: FnMut(D) -> S,
    D: DeserializeOwned,
    S: Serialize,
{
    let mut handler = handler;
    run(|event| Result::Ok::<_, std::convert::Infallible>(handler(event)))
}

fn run_inner<F, D, S, E>(addr: SocketAddr, handler: &mut F) -> std::io::Result<()>
where
    F: FnMut(D) -> Result<S, E>,
    D: DeserializeOwned,
    S: Serialize,
    E: std::error::Error + 'static,
{
    http::get(addr, "invocation/next").and_then(|(request_id, body)| match handler(body) {
        Ok(response) => http::post(
            addr,
            &format!("invocation/{}/response", request_id),
            &response,
        ),
        Err(err) => http::post_error(
            addr,
            &format!("invocation/{}/error", request_id),
            std::any::type_name::<E>(),
            &err.to_string(),
        ),
    })
}
