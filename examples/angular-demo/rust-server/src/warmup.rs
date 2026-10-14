use std::{env, num::NonZeroU64};

use deno_core::{OpState, op2};

const REQUESTS_VARIABLE: &str = "RUBICON_DEMO_WARMUP_REQUESTS";

pub(crate) fn requests_from_environment() -> Result<Option<NonZeroU64>, String> {
    match env::var(REQUESTS_VARIABLE) {
        Ok(value) => parse_requests(&value).map(Some),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(format!("Cannot read {REQUESTS_VARIABLE}: {error}")),
    }
}

fn parse_requests(value: &str) -> Result<NonZeroU64, String> {
    value
        .parse::<u64>()
        .ok()
        .and_then(NonZeroU64::new)
        .ok_or_else(|| format!("{REQUESTS_VARIABLE} must be a positive integer."))
}

struct WarmupCounter {
    runtime_id: usize,
    target: NonZeroU64,
    remaining: u64,
}

impl WarmupCounter {
    fn new(runtime_id: usize, target: NonZeroU64) -> Self {
        Self {
            runtime_id,
            target,
            remaining: target.get(),
        }
    }

    fn record_completed_render(&mut self) -> Option<String> {
        if self.remaining == 0 {
            return None;
        }

        self.remaining -= 1;

        (self.remaining == 0).then(|| {
            format!(
                "RUBICON_DEMO_WARMUP_READY runtime_id={} completed={}",
                self.runtime_id, self.target,
            )
        })
    }
}

#[op2(fast)]
fn op_demo_warmup_complete(state: &mut OpState) -> bool {
    let counter = state.borrow_mut::<WarmupCounter>();

    if let Some(confirmation) = counter.record_completed_render() {
        eprintln!("{confirmation}");
    }

    counter.remaining == 0
}

deno_core::extension!(
    rubicon_demo_warmup,
    ops = [op_demo_warmup_complete],
    esm_entry_point = "ext:rubicon_demo_warmup/demo_warmup.js",
    esm = [dir "js", "demo_warmup.js"],
    options = {
        runtime_id: usize,
        requests: NonZeroU64,
    },
    state = |state, options| {
        state.put(WarmupCounter::new(options.runtime_id, options.requests));
    },
);

pub(crate) fn extension(runtime_id: usize, requests: NonZeroU64) -> deno_core::Extension {
    rubicon_demo_warmup::init(runtime_id, requests)
}

#[cfg(test)]
mod tests {
    use super::{WarmupCounter, extension, parse_requests};
    use rubicon::JavaScriptRuntime;

    #[test]
    fn warmup_threshold_rejects_invalid_values() {
        assert_eq!(parse_requests("20").unwrap().get(), 20);

        for value in ["", "0", "-1", "1.5", "abc", "18446744073709551616"] {
            assert!(
                parse_requests(value).is_err(),
                "Accepted invalid threshold: {value}"
            );
        }
    }

    #[test]
    fn warmup_confirmation_is_emitted_once_per_runtime() {
        let target = parse_requests("2").unwrap();
        let mut first = WarmupCounter::new(1, target);
        let mut second = WarmupCounter::new(2, target);

        assert!(first.record_completed_render().is_none());
        assert_eq!(
            first.record_completed_render().as_deref(),
            Some("RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2"),
        );
        assert!(first.record_completed_render().is_none());
        assert!(second.record_completed_render().is_none());
        assert_eq!(
            second.record_completed_render().as_deref(),
            Some("RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=2"),
        );
        assert!(second.record_completed_render().is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn warmup_hook_is_optional_and_has_independent_javascript_state() {
        deno_core::JsRuntime::init_platform(None);

        {
            let mut runtime = JavaScriptRuntime::new().unwrap();
            assert_eq!(
                runtime
                    .execute_i64("Number(typeof globalThis.__rubiconDemoWarmup === 'undefined')")
                    .unwrap(),
                1,
            );
        }

        for runtime_id in 1..=2 {
            let mut runtime = JavaScriptRuntime::with_extensions(vec![extension(
                runtime_id,
                parse_requests("2").unwrap(),
            )])
            .unwrap();

            for expected in [0, 1, 1] {
                assert_eq!(
                    runtime
                        .execute_i64(
                            "Number(globalThis.__rubiconDemoWarmup.recordCompletedRender())"
                        )
                        .unwrap(),
                    expected,
                    "Unexpected warmup state in runtime {runtime_id}",
                );
            }
        }
    }
}
