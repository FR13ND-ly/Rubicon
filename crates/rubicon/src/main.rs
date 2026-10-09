use rubicon::JavaScriptRuntime;

fn main() -> Result<(), String> {
    let mut runtime = JavaScriptRuntime::new()?;

    runtime.execute_i64("globalThis.counter = 0")?;

    let first = runtime.execute_i64("++globalThis.counter")?;
    let second = runtime.execute_i64("++globalThis.counter")?;

    println!("First execution: {first}");
    println!("Second execution: {second}");

    Ok(())
}
