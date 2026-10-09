use deno_core::{JsRuntime, RuntimeOptions, serde_v8, v8};

pub struct JavaScriptRuntime {
    inner: JsRuntime,
}

impl JavaScriptRuntime {
    pub fn new() -> Result<Self, String> {
        let inner = JsRuntime::try_new(RuntimeOptions::default())
            .map_err(|error| format!("Runtime initialization failed: {error}"))?;

        Ok(Self { inner })
    }

    pub fn execute_i64(&mut self, source: &str) -> Result<i64, String> {
        let global = self
            .inner
            .execute_script("rubicon-experiment.js", source.to_owned())
            .map_err(|error| format!("JavaScript execution failed: {error}"))?;

        deno_core::scope!(scope, &mut self.inner);
        let local = v8::Local::new(scope, global);

        serde_v8::from_v8::<i64>(scope, local)
            .map_err(|error| format!("Result conversion failed: {error}"))
    }
}
