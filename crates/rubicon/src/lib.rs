use deno_core::{JsRuntime, RuntimeOptions, serde_v8, v8};

pub fn execute_javascript(source: &str) -> Result<i64, String> {
    let mut runtime = JsRuntime::try_new(RuntimeOptions::default())
        .map_err(|error| format!("Runtime initialization failed: {error}"))?;

    let global = runtime
        .execute_script("rubicon-experiment.js", source.to_owned())
        .map_err(|error| format!("JavaScript execution failed: {error}"))?;

    deno_core::scope!(scope, &mut runtime);
    let local = v8::Local::new(scope, global);

    serde_v8::from_v8::<i64>(scope, local)
        .map_err(|error| format!("Result conversion failed: {error}"))
}
