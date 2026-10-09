use std::path::Path;

use rubicon::JavaScriptRuntime;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let mut runtime = JavaScriptRuntime::new()?;

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("js/render.mjs");
    let module_id = runtime.load_main_module(&path).await?;

    let first_html = runtime.call_string_export(module_id, "render").await?;
    let second_html = runtime.call_string_export(module_id, "render").await?;

    println!("First render: {first_html}");
    println!("Second render: {second_html}");

    Ok(())
}
