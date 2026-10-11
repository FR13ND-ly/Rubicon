use deno_core::op2;

#[op2(fast)]
fn op_rubicon_adapter_timing(stage: u32, elapsed_ms: f64, ok: bool) -> bool {
    let stage = match stage {
        1 => "angular_handle",
        2 => "body_read",
        _ => return false,
    };

    let elapsed_us = elapsed_ms * 1000.0;
    if !elapsed_us.is_finite() || elapsed_us < 0.0 || elapsed_us >= u64::MAX as f64 {
        return false;
    }

    tracing::info!(
        target: "rubicon::adapter_timing",
        stage,
        elapsed_us = elapsed_us.floor() as u64,
        ok,
        "adapter_stage_timing",
    );

    true
}

deno_core::extension!(
    rubicon_diagnostics,
    deps = [rubicon_web],
    ops = [op_rubicon_adapter_timing],
    esm_entry_point = "ext:rubicon_diagnostics/diagnostics.js",
    esm = [dir "js", "diagnostics.js"],
);

pub(crate) fn extension() -> deno_core::Extension {
    rubicon_diagnostics::init()
}
