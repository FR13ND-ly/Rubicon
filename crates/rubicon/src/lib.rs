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

#[cfg(test)]
mod tests {
    use super::JavaScriptRuntime;

    #[test]
    fn runtime_recovers_after_execution_and_conversion_errors() -> Result<(), String> {
        let mut runtime = JavaScriptRuntime::new()?;

        assert_eq!(runtime.execute_i64("globalThis.counter = 0")?, 0);

        let execution_error = runtime
            .execute_i64("throw new Error('intentional test error')")
            .expect_err("A JavaScript exception should return Err");

        assert!(execution_error.starts_with("JavaScript execution failed:"));

        assert_eq!(runtime.execute_i64("++globalThis.counter")?, 1);

        let conversion_error = runtime
            .execute_i64("({ value: 42 })")
            .expect_err("An object result should fail conversion to i64");

        assert!(conversion_error.starts_with("Result conversion failed:"));

        assert_eq!(runtime.execute_i64("++globalThis.counter")?, 2);

        Ok(())
    }
}
