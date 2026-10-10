use std::{path::Path, rc::Rc, sync::Arc};

use deno_core::{
    FsModuleLoader, JsRuntime, ModuleId, ModuleSpecifier, RuntimeOptions, serde_v8, v8,
};

deno_core::extension!(
    rubicon_web,
    deps = [deno_webidl, deno_web],
    esm_entry_point = "ext:rubicon_web/web.js",
    esm = [dir "js", "web.js"],
);
mod http_response;
mod renderer;
mod request;
mod response;

pub use http_response::into_http_response;
pub use renderer::{RenderError, Renderer};
pub use request::RenderRequest;
pub use response::RenderResponse;

pub struct JavaScriptRuntime {
    inner: JsRuntime,
}

impl JavaScriptRuntime {
    pub fn new() -> Result<Self, String> {
        let inner = JsRuntime::try_new(RuntimeOptions {
            module_loader: Some(Rc::new(FsModuleLoader)),
            extensions: vec![
                deno_webidl::deno_webidl::init(),
                deno_web::deno_web::init(
                    Arc::new(deno_web::BlobStore::default()),
                    None,
                    false,
                    Default::default(),
                ),
                rubicon_web::init(),
            ],
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

    pub async fn call_export<TArgument, TResult>(
        &mut self,
        module_id: ModuleId,
        export_name: &str,
        argument: &TArgument,
    ) -> Result<TResult, String>
    where
        TArgument: serde::Serialize,
        TResult: serde::de::DeserializeOwned,
    {
        let namespace = self
            .inner
            .get_module_namespace(module_id)
            .map_err(|error| format!("Module namespace access failed: {error}"))?;

        let (function, argument) = {
            deno_core::scope!(scope, &mut self.inner);
            let namespace = v8::Local::new(scope, namespace);

            let key = v8::String::new(scope, export_name)
                .ok_or_else(|| "Cannot allocate export name".to_owned())?;

            let value = namespace
                .get(scope, key.into())
                .ok_or_else(|| format!("Cannot read export: {export_name}"))?;

            let function = v8::Local::<v8::Function>::try_from(value)
                .map_err(|_| format!("Export {export_name} must be a function"))?;

            let argument = serde_v8::to_v8(scope, argument)
                .map_err(|error| format!("Argument conversion failed: {error}"))?;

            (
                v8::Global::new(scope, function),
                v8::Global::new(scope, argument),
            )
        };

        let arguments = [argument];
        let call = Box::pin(self.inner.call_with_args(&function, &arguments));

        let result = self
            .inner
            .with_event_loop_promise(call, Default::default())
            .await
            .map_err(|error| format!("Export execution failed: {error}"))?;

        deno_core::scope!(scope, &mut self.inner);
        let local = v8::Local::new(scope, result);

        serde_v8::from_v8::<TResult>(scope, local)
            .map_err(|error| format!("Result conversion failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::JavaScriptRuntime;

    #[tokio::test(flavor = "current_thread")]
    async fn runtime_recovers_after_execution_and_conversion_errors() -> Result<(), String> {
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
    #[tokio::test(flavor = "current_thread")]
    async fn runtime_supports_url_and_search_parameters() -> Result<(), String> {
        let mut runtime = JavaScriptRuntime::new()?;

        let result = runtime.execute_i64(
            r#"
            (() => {
                const url = new URL(
                    "/search?q=Rust%20SSR&page=2&page=3",
                    "http://127.0.0.1:3000",
                );

                if (url.pathname !== "/search") throw new Error("Wrong URL path");
                if (url.searchParams.get("q") !== "Rust SSR") {
                    throw new Error("Query decoding failed");
                }
                if (url.searchParams.getAll("page").join(",") !== "2,3") {
                    throw new Error("Repeated parameters were lost");
                }

                const parameters = new URLSearchParams();
                parameters.set("message", "țară & rust");
                const decoded = new URLSearchParams(parameters.toString());

                if (decoded.get("message") !== "țară & rust") {
                    throw new Error("Query round trip failed");
                }

                return 1;
            })()
            "#,
        )?;

        assert_eq!(result, 1);
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn runtime_supports_utf8_encoding_and_decoding() -> Result<(), String> {
        let mut runtime = JavaScriptRuntime::new()?;

        let result = runtime.execute_i64(
            r#"
            (() => {
                const text = "Salut, țară 🌍";
                const bytes = new TextEncoder().encode(text);

                if (!(bytes instanceof Uint8Array)) {
                    throw new Error("Encoder did not return bytes");
                }
                if (new TextDecoder().decode(bytes) !== text) {
                    throw new Error("UTF-8 round trip failed");
                }

                let rejected = false;
                try {
                    new TextDecoder("utf-8", { fatal: true })
                        .decode(new Uint8Array([0xff]));
                } catch (error) {
                    if (!(error instanceof TypeError)) throw error;
                    rejected = true;
                }

                if (!rejected) throw new Error("Invalid UTF-8 was accepted");
                return 1;
            })()
            "#,
        )?;

        assert_eq!(result, 1);
        Ok(())
    }
}
