# minlambda

A minimalist [AWS Lambda][lambda] [runtime] for Rust.

```rust
fn main() -> ! {
    minlambda::run_ok(|_: serde::de::IgnoredAny| "Hello, world!")
}
```

[lambda]: https://aws.amazon.com/lambda/
[runtime]: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html

## What it does

minlambda implements the [AWS Lambda runtime interface][interface], deserializing events and
serializing responses with [Serde JSON][`serde_json`].

To communicate with the runtime API over HTTP, minlambda uses a purpose-built HTTP client.

[interface]: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html

## What it doesn't

minlambda doesn't parse [response headers in the invocation event][next] (other than the
request ID). This includes the function deadline, function ARN, AWS X-Ray tracing header, or
additional AWS Mobile SDK data. The crate author has never needed these and, well, this is a
minimal runtime.

minlambda doesn't run your handler in an async runtime. If you're using async code, you can
create a runtime outside of `lambda::run` and call its blocking function (e.g. Tokio's
`Runtime::block_on`). [An example for Tokio is available.][tokio-example]

[next]: https://docs.aws.amazon.com/lambda/latest/dg/runtimes-api.html#runtimes-api-next
[tokio-example]: https://github.com/iliana/minlambda/blob/matriarch/examples/async.rs

## When not to use this

Probably most of the time.

If you're using Lambda to interact with other AWS services, which is very likely, you are
probably using an SDK (such as [Rusoto]) that probably relies on [hyper] and [Tokio], and
you're not really reducing your total dependency closure compared to the [AWS Labs
runtime][awslabs].

The HTTP client was built to work with Lambda's runtime API, and not to be a generic
RFC-compliant HTTP client; if Lambda's underlying interface subtly changes, this runtime could
break unexpectedly. (This probably won't happen: we believe that the subset of the HTTP spec we
implement is by the book.)

[Rusoto]: https://github.com/rusoto/rusoto
[hyper]: https://docs.rs/hyper
[tokio]: https://docs.rs/tokio
[awslabs]: https://github.com/awslabs/aws-lambda-rust-runtime

## When to use this

You like simple things, or your code already has minimal dependencies.

## Examples

[Some lovely examples are available in our repository.][examples]

[examples]: https://github.com/iliana/minlambda/tree/matriarch/examples

## Building Lambda functions

Building binaries that actually work in the Lambda execution environment is a bit of an art, as
it contains stable (old) versions of glibc and the like. Your compiler is probably targeting a
system with newer shared libraries and symbol versions than what the execution environment has
available, resulting in [cryptic dynamic linker errors at runtime][cryptic].

If you find the musl libc toolchain reasonable to work with, [building a fully static binary is
probably the way to go][musl]. If you find containers reasonable to work with, [using
softprops/lambda-rust is probably the way to go][container].

[cryptic]: https://github.com/awslabs/aws-lambda-rust-runtime/issues/17
[musl]: https://doc.rust-lang.org/edition-guide/rust-2018/platform-and-target-support/musl-support-for-fully-static-binaries.html
[container]: https://github.com/softprops/lambda-rust

## Disclaimer

The author of this crate works at AWS, but this is not an official AWS project, nor does it
necessarily represent opinions of or recommended best-practices on AWS.
