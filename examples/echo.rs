// This Lambda function just parses whatever the event value is and spits it back out again.

fn main() {
    minlambda::run_ok(|value: serde_json::Value| value)
}
