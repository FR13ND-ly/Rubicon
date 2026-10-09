use std::{path::Path, rc::Rc};

use deno_core::{
    FsModuleLoader, JsRuntime, ModuleId, ModuleSpecifier, RuntimeOptions, serde_v8, v8,
};

pub struct JavaScriptRuntime {
    inner: JsRuntime,
}

impl JavaScriptRuntime {
    pub fn new() -> Result<Self, String> {
        let inner = JsRuntime::try_new(RuntimeOptions {
            module_loader: Some(Rc::new(FsModuleLoader)),
            ..Default::default()
        })
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

impl JavaScriptRuntime {
    pub async fn load_main_module(&mut self, path: &Path) -> Result<ModuleId, String> {
        let absolute_path = path
            .canonicalize()
            .map_err(|error| format!("Module path resolution failed: {error}"))?;

        let specifier = ModuleSpecifier::from_file_path(&absolute_path)
            .map_err(|_| "Cannot convert module path to a file URL".to_owned())?;

        let module_id = self
            .inner
            .load_main_es_module(&specifier)
            .await
            .map_err(|error| format!("Module loading failed: {error}"))?;

        let evaluation = self.inner.mod_evaluate(module_id);

        self.inner
            .run_event_loop(Default::default())
            .await
            .map_err(|error| format!("Module event loop failed: {error}"))?;

        evaluation
            .await
            .map_err(|error| format!("Module evaluation failed: {error}"))?;

        Ok(module_id)
    }

    pub async fn call_string_export(
        &mut self,
        module_id: ModuleId,
        export_name: &str,
    ) -> Result<String, String> {
        let namespace = self
            .inner
            .get_module_namespace(module_id)
            .map_err(|error| format!("Module namespace access failed: {error}"))?;

        let function = {
            deno_core::scope!(scope, &mut self.inner);
            let namespace = v8::Local::new(scope, namespace);

            let key = v8::String::new(scope, export_name)
                .ok_or_else(|| "Cannot allocate export name".to_owned())?;

            let value = namespace
                .get(scope, key.into())
                .ok_or_else(|| format!("Cannot read export: {export_name}"))?;

            let function = v8::Local::<v8::Function>::try_from(value)
                .map_err(|_| format!("Export {export_name} must be a function"))?;

            v8::Global::new(scope, function)
        };

        let call = Box::pin(self.inner.call(&function));

        let result = self
            .inner
            .with_event_loop_promise(call, Default::default())
            .await
            .map_err(|error| format!("Export execution failed: {error}"))?;

        deno_core::scope!(scope, &mut self.inner);
        let local = v8::Local::new(scope, result);

        serde_v8::from_v8::<String>(scope, local)
            .map_err(|error| format!("String conversion failed: {error}"))
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
