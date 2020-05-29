// Copyright (c) 2020 iliana destroyer of worlds <iliana@buttslol.net>
// SPDX-License-Identifier: MIT

//! A minimalist [AWS Lambda][lambda] [runtime] for Rust.
//!
//! ```rust,no_run
//! fn main() -> ! {
//!     minlambda::run_ok(|_: serde::de::IgnoredAny| "Hello, world!")
//! }
//! ```
//!
//! [lambda]: https://aws.amazon.com/lambda/
//! [runtime]: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html
//!
//! # What it does
//!
//! minlambda implements the [AWS Lambda runtime interface][interface], deserializing events and
//! serializing responses with [Serde JSON][`serde_json`].
//!
//! To communicate with the runtime API over HTTP, minlambda uses a purpose-built HTTP client.
//!
//! [interface]: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html
//!
//! # What it doesn't
//!
//! minlambda doesn't parse [response headers in the invocation event][next] (other than the
//! request ID). This includes the function deadline, function ARN, AWS X-Ray tracing header, or
//! additional AWS Mobile SDK data. The crate author has never needed these and, well, this is a
//! minimal runtime.
//!
//! minlambda doesn't run your handler in an async runtime. If you're using async code, you can
//! create a runtime outside of `lambda::run` and call its blocking function (e.g. Tokio's
//! `Runtime::block_on`). [An example for Tokio is available.][tokio-example]
//!
//! [next]: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-next
//! [tokio-example]: https://github.com/iliana/minlambda/blob/matriarch/examples/async.rs
//!
//! # When not to use this
//!
//! Probably most of the time.
//!
//! If you're using Lambda to interact with other AWS services, which is very likely, you are
//! probably using an SDK (such as [Rusoto]) that probably relies on [hyper] and [Tokio], and
//! you're not really reducing your total dependency closure compared to the [AWS Labs
//! runtime][awslabs].
//!
//! The HTTP client was built to work with Lambda, and not to be a generic RFC-compliant HTTP
//! client; if the underlying protocol subtly changes, this runtime could break unexpectedly. (This
//! probably won't happen: we believe that the subset of the HTTP spec we *do* implement is by the
//! book.)
//!
//! [Rusoto]: https://github.com/rusoto/rusoto
//! [hyper]: https://docs.rs/hyper
//! [tokio]: https://docs.rs/tokio
//! [awslabs]: https://github.com/awslabs/aws-lambda-rust-runtime
//!
//! # When to use this
//!
//! You like simple things, or your code already has minimal dependencies.
//!
//! # Examples
//!
//! [Some lovely examples are available in our repository.][examples]
//!
//! [examples]: https://github.com/iliana/minlambda/tree/matriarch/examples
//!
//! # Building Lambda functions
//!
//! Building binaries that actually work in the Lambda execution environment is a bit of an art, as
//! it contains stable (old) versions of glibc and the like. Your compiler is probably targeting a
//! system with newer shared libraries and symbol versions than what the execution environment has
//! available, resulting in [cryptic dynamic linker errors at runtime][cryptic].
//!
//! If you find the musl libc toolchain reasonable to work with, [building a fully static binary is
//! probably the way to go][musl]. If you find containers reasonable to work with, [using
//! softprops/lambda-rust is probably the way to go][container].
//!
//! [cryptic]: https://github.com/awslabs/aws-lambda-rust-runtime/issues/17
//! [musl]: https://doc.rust-lang.org/edition-guide/rust-2018/platform-and-target-support/musl-support-for-fully-static-binaries.html
//! [container]: https://github.com/softprops/lambda-rust
//!
//! # Disclaimer
//!
//! The author of this crate works at AWS, but this is not an official AWS project, nor does it
//! necessarily represent opinions of AWS.

#![forbid(unsafe_code)]
#![deny(
    future_incompatible,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unused
)]
#![warn(clippy::pedantic)]

mod http;

use serde::{de::DeserializeOwned, Serialize};
use std::net::SocketAddr;

/// Retrieves invocation events, calls your handler, and sends back response data within the Lambda
/// execution environment.
///
/// This function [does not return][diverging] (Lambda will kill processes when unused).
///
/// # Panics
///
/// This function panics on two fatal error conditions:
///
/// * Failing to parse the `AWS_LAMBDA_RUNTIME_API` environment variable as a [`SocketAddr`].
/// * Failing to report an error to the runtime interface.
///
/// [diverging]: https://doc.rust-lang.org/stable/rust-by-example/fn/diverging.html
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

/// [`run`], for handlers that don't return [`Result`].
///
/// This function is otherwise the same as `run`: it does not return and will panic on certain
/// unrecoverable errors.
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
