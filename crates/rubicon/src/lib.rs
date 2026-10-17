use std::{
    future::Future,
    path::Path,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
};

use deno_core::{
    FsModuleLoader, JsRuntime, ModuleId, ModuleSpecifier, RuntimeOptions, serde_v8, v8,
};

deno_core::extension!(
    rubicon_web,
    deps = [deno_webidl, deno_web, deno_net, deno_fetch],
    esm_entry_point = "ext:rubicon_web/web.js",
    esm = [dir "js", "web.js"],
);
#[cfg(feature = "render-timing")]
mod diagnostics;

mod config;
mod http_response;
mod renderer;
mod request;
mod response;
mod server;
mod static_files;
mod streaming;

#[cfg(windows)]
mod windows_timer;

pub use config::{RendererConfig, ServerConfig};
pub use http_response::{into_http_response, into_http_streaming_response};
pub use renderer::{RenderError, Renderer};
pub use request::RenderRequest;
pub use response::{RenderBody, RenderContent, RenderResponse};
pub use server::serve;
pub use streaming::RenderStream;

#[cfg(windows)]
pub use windows_timer::TimerResolutionGuard;

pub(crate) type PendingExport =
    Pin<Box<dyn Future<Output = Result<v8::Global<v8::Value>, String>>>>;
pub(crate) type PreparedResponse = (
    RenderResponse<RenderContent>,
    Option<streaming::StreamCompletion>,
);
pub(crate) struct RenderBridge {
    pub(crate) function: v8::Global<v8::Function>,
    #[cfg(feature = "streaming")]
    open: v8::Global<v8::Function>,
    #[cfg(feature = "streaming")]
    cancel: v8::Global<v8::Function>,
    #[cfg(feature = "streaming")]
    body_key: v8::Global<v8::String>,
}

pub struct JavaScriptRuntime {
    inner: JsRuntime,
}

impl JavaScriptRuntime {
    pub fn new() -> Result<Self, String> {
        Self::with_extensions(Vec::new())
    }

    pub fn with_extensions(
        application_extensions: Vec<deno_core::Extension>,
    ) -> Result<Self, String> {
        let mut extensions = vec![
            deno_webidl::deno_webidl::init(),
            deno_web::deno_web::init(
                Arc::new(deno_web::BlobStore::default()),
                None,
                false,
                Default::default(),
            ),
            deno_net::deno_net::init(None, None),
            deno_fetch::deno_fetch::init(Default::default()),
            rubicon_web::init(),
        ];

        #[cfg(feature = "streaming")]
        extensions.push(streaming::extension());

        #[cfg(feature = "render-timing")]
        extensions.push(diagnostics::extension());

        extensions.extend(application_extensions);

        let inner = JsRuntime::try_new(RuntimeOptions {
            module_loader: Some(Rc::new(FsModuleLoader)),
            extensions,
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
    pub(crate) fn poll_event_loop(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), String>> {
        self.inner
            .poll_event_loop(cx, Default::default())
            .map_err(|error| format!("Background event loop failed: {error}"))
    }
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
        let (function, argument) = self.prepare_export(module_id, export_name, argument)?;

        #[cfg(feature = "render-timing")]
        let execute_started = std::time::Instant::now();

        let arguments = [argument];
        let call = Box::pin(self.inner.call_with_args(&function, &arguments));

        let executed = self
            .inner
            .with_event_loop_promise(call, Default::default())
            .await
            .map_err(|error| format!("Export execution failed: {error}"));

        #[cfg(feature = "render-timing")]
        {
            let elapsed_us = execute_started.elapsed().as_micros();

            tracing::info!(
                target: "rubicon::runtime_timing",
                stage = "execute",
                elapsed_us,
                ok = executed.is_ok(),
                "runtime_stage_timing",
            );
        }

        let result = executed?;

        self.convert_result(result)
    }
    fn prepare_export<TArgument: serde::Serialize>(
        &mut self,
        module_id: ModuleId,
        export_name: &str,
        argument: &TArgument,
    ) -> Result<(v8::Global<v8::Function>, v8::Global<v8::Value>), String> {
        #[cfg(feature = "render-timing")]
        let prepare_started = std::time::Instant::now();

        let prepared = (|| {
            let namespace = self
                .inner
                .get_module_namespace(module_id)
                .map_err(|error| format!("Module namespace access failed: {error}"))?;

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

            Ok::<_, String>((
                v8::Global::new(scope, function),
                v8::Global::new(scope, argument),
            ))
        })();

        #[cfg(feature = "render-timing")]
        {
            let elapsed_us = prepare_started.elapsed().as_micros();

            tracing::info!(
                target: "rubicon::runtime_timing",
                stage = "prepare",
                elapsed_us,
                ok = prepared.is_ok(),
                "runtime_stage_timing",
            );
        }

        prepared
    }

    pub(crate) fn begin_export<TArgument: serde::Serialize>(
        &mut self,
        function: &v8::Global<v8::Function>,
        argument: TArgument,
    ) -> Result<PendingExport, String> {
        #[cfg(feature = "render-timing")]
        let prepare_started = std::time::Instant::now();
        let serialized = {
            deno_core::scope!(scope, &mut self.inner);
            let argument = serde_v8::to_v8(scope, &argument)
                .map_err(|error| format!("Argument conversion failed: {error}"))?;
            v8::Global::new(scope, argument)
        };
        drop(argument);
        #[cfg(feature = "render-timing")]
        tracing::info!(target: "rubicon::runtime_timing", stage = "prepare",
            elapsed_us = prepare_started.elapsed().as_micros(), ok = true,
            "runtime_stage_timing");
        #[cfg(feature = "render-timing")]
        let execute_started = std::time::Instant::now();
        // call_with_args starts the export now; its owned future is completed by
        // later event-loop polls without retaining a mutable runtime borrow.
        let future = self.inner.call_with_args(function, &[serialized]);
        Ok(Box::pin(async move {
            let result = future
                .await
                .map_err(|error| format!("Export execution failed: {error}"));
            #[cfg(feature = "render-timing")]
            tracing::info!(
                target: "rubicon::runtime_timing",
                stage = "execute",
                elapsed_us = execute_started.elapsed().as_micros(),
                ok = result.is_ok(),
                "runtime_stage_timing",
            );
            result
        }))
    }

    pub(crate) fn resolve_export(
        &mut self,
        module_id: ModuleId,
        name: &str,
    ) -> Result<v8::Global<v8::Function>, String> {
        let namespace = self
            .inner
            .get_module_namespace(module_id)
            .map_err(|error| format!("Module namespace access failed: {error}"))?;
        deno_core::scope!(scope, &mut self.inner);
        let namespace = v8::Local::new(scope, namespace);
        let key =
            v8::String::new(scope, name).ok_or_else(|| "Cannot allocate export name".to_owned())?;
        let value = namespace
            .get(scope, key.into())
            .ok_or_else(|| format!("Cannot read export: {name}"))?;
        let function = v8::Local::<v8::Function>::try_from(value)
            .map_err(|_| format!("Export {name} must be a function"))?;
        Ok(v8::Global::new(scope, function))
    }

    pub(crate) fn prepare_renderer(&mut self, module_id: ModuleId) -> Result<RenderBridge, String> {
        let render = self.resolve_export(module_id, "render")?;
        #[cfg(not(feature = "streaming"))]
        return Ok(RenderBridge { function: render });
        #[cfg(feature = "streaming")]
        {
            let open = self
                .inner
                .execute_script("rubicon-stream-open.js", "globalThis.__rubiconOpenStream")
                .map_err(|error| format!("Renderer bridge initialization failed: {error}"))?;
            let cancel = self
                .inner
                .execute_script(
                    "rubicon-stream-cancel.js",
                    "globalThis.__rubiconCancelStream",
                )
                .map_err(|error| format!("Renderer bridge initialization failed: {error}"))?;
            deno_core::scope!(scope, &mut self.inner);
            let open = v8::Local::<v8::Function>::try_from(v8::Local::new(scope, open))
                .map_err(|_| "Renderer bridge must be a function".to_owned())?;
            let cancel = v8::Local::<v8::Function>::try_from(v8::Local::new(scope, cancel))
                .map_err(|_| "Renderer bridge must be a function".to_owned())?;
            let body_key = v8::String::new(scope, "body").ok_or("Cannot allocate body key")?;
            Ok(RenderBridge {
                function: render,
                open: v8::Global::new(scope, open),
                cancel: v8::Global::new(scope, cancel),
                body_key: v8::Global::new(scope, body_key),
            })
        }
    }

    #[cfg(not(feature = "streaming"))]
    pub(crate) fn convert_render_response(
        &mut self,
        value: v8::Global<v8::Value>,
        wanted: bool,
        _bridge: &RenderBridge,
    ) -> Result<Option<PreparedResponse>, String> {
        if !wanted {
            return Ok(None);
        }
        let response: RenderResponse = self.convert_result(value)?;
        Ok(Some((
            RenderResponse {
                status: response.status,
                headers: response.headers,
                body: RenderContent::Buffered(response.body),
            },
            None,
        )))
    }

    #[cfg(feature = "streaming")]
    pub(crate) fn convert_render_response(
        &mut self,
        value: v8::Global<v8::Value>,
        wanted: bool,
        bridge: &RenderBridge,
    ) -> Result<Option<PreparedResponse>, String> {
        #[derive(serde::Deserialize)]
        struct Metadata {
            status: u16,
            headers: Vec<(String, String)>,
        }
        enum Body {
            Buffered(RenderBody),
            Stream(u32),
        }
        #[cfg(feature = "render-timing")]
        let started = std::time::Instant::now();
        let converted = (|| {
            let (metadata, body) = {
                deno_core::scope!(scope, &mut self.inner);
                v8::tc_scope!(let scope, scope);
                let local = v8::Local::new(scope, value);
                let object = v8::Local::<v8::Object>::try_from(local)
                    .map_err(|_| "Result conversion failed: expected response object".to_owned())?;
                let metadata: Option<Metadata> = if wanted {
                    Some(
                        serde_v8::from_v8(scope, local)
                            .map_err(|error| format!("Result conversion failed: {error}"))?,
                    )
                } else {
                    None
                };
                let key = v8::Local::new(scope, &bridge.body_key);
                let body = object
                    .get(scope, key.into())
                    .ok_or("Result conversion failed: cannot read response body")?;
                let buffered =
                    body.is_string() || body.is_array_buffer() || body.is_array_buffer_view();
                if !wanted {
                    if !buffered {
                        let cancel = v8::Local::new(scope, &bridge.cancel);
                        let receiver = v8::undefined(scope).into();
                        let _ = cancel.call(scope, receiver, &[body]);
                    }
                    return Ok(None);
                }
                let body = if buffered {
                    Body::Buffered(
                        serde_v8::from_v8(scope, body)
                            .map_err(|error| format!("Result conversion failed: {error}"))?,
                    )
                } else {
                    let open = v8::Local::new(scope, &bridge.open);
                    let receiver = v8::undefined(scope).into();
                    let id = open
                        .call(scope, receiver, &[body])
                        .and_then(|value| value.uint32_value(scope))
                        .ok_or("Result conversion failed: cannot open response stream")?;
                    Body::Stream(id)
                };
                (metadata.expect("Wanted responses have metadata"), body)
            };
            let (body, completion) = match body {
                Body::Stream(id) => {
                    let (stream, completion) = streaming::take(&mut self.inner, id)?;
                    (RenderContent::Stream(stream), Some(completion))
                }
                Body::Buffered(body) => (RenderContent::Buffered(body), None),
            };
            Ok(Some((
                RenderResponse {
                    status: metadata.status,
                    headers: metadata.headers,
                    body,
                },
                completion,
            )))
        })();
        #[cfg(feature = "render-timing")]
        if wanted {
            tracing::info!(target: "rubicon::runtime_timing", stage = "convert",
                elapsed_us = started.elapsed().as_micros(), ok = converted.is_ok(), "runtime_stage_timing");
        };
        converted
    }

    pub(crate) fn convert_result<TResult: serde::de::DeserializeOwned>(
        &mut self,
        result: v8::Global<v8::Value>,
    ) -> Result<TResult, String> {
        #[cfg(feature = "render-timing")]
        let convert_started = std::time::Instant::now();

        let converted = {
            deno_core::scope!(scope, &mut self.inner);
            let local = v8::Local::new(scope, result);

            serde_v8::from_v8::<TResult>(scope, local)
                .map_err(|error| format!("Result conversion failed: {error}"))
        };

        #[cfg(feature = "render-timing")]
        {
            let elapsed_us = convert_started.elapsed().as_micros();

            tracing::info!(
                target: "rubicon::runtime_timing",
                stage = "convert",
                elapsed_us,
                ok = converted.is_ok(),
                "runtime_stage_timing",
            );
        }

        converted
    }
}

#[cfg(test)]
mod tests {
    use super::JavaScriptRuntime;
    use deno_core::OpState;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[deno_core::op2(fast)]
    fn op_rust_argument_was_dropped(state: &mut OpState) -> bool {
        state.borrow::<Arc<AtomicBool>>().load(Ordering::SeqCst)
    }

    deno_core::extension!(argument_drop_probe,
        ops = [op_rust_argument_was_dropped],
        options = { dropped: Arc<AtomicBool>, },
        state = |state, options| { state.put(options.dropped); },
    );

    #[tokio::test(flavor = "current_thread")]
    async fn owned_arguments_are_released_before_synchronous_javascript_runs() -> Result<(), String>
    {
        #[derive(serde::Serialize)]
        struct Argument {
            value: i64,
            #[serde(skip)]
            dropped: Arc<AtomicBool>,
        }
        impl Drop for Argument {
            fn drop(&mut self) {
                self.dropped.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let mut runtime = JavaScriptRuntime::with_extensions(vec![argument_drop_probe::init(
            Arc::clone(&dropped),
        )])?;
        let function = runtime.inner.execute_script("argument-drop-probe.js",
            "(argument) => { if (!Deno.core.ops.op_rust_argument_was_dropped()) throw new Error('Rust argument retained'); return argument.value; }")
            .map_err(|error| error.to_string())?;
        let function = {
            deno_core::scope!(scope, &mut runtime.inner);
            let function = deno_core::v8::Local::new(scope, function);
            let function =
                deno_core::v8::Local::<deno_core::v8::Function>::try_from(function).unwrap();
            deno_core::v8::Global::new(scope, function)
        };
        let result = runtime
            .begin_export(&function, Argument { value: 7, dropped })?
            .await?;
        assert_eq!(runtime.convert_result::<i64>(result)?, 7);
        Ok(())
    }

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
