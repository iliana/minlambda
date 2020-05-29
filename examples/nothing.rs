// This handler does nothing. It's here as a good example for ignoring the input event.

fn main() {
    minlambda::run_ok(|_: serde::de::IgnoredAny| ())
}
