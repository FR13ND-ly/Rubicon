fn main() -> Result<(), String> {
    let result = rubicon::execute_javascript("4 + 2")?;

    println!("JavaScript result: {result}");

    Ok(())
}
