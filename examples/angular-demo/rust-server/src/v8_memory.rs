use std::env;

const VARIABLE: &str = "RUBICON_DEMO_V8_SEMI_SPACE_MIB";

fn parse_mib(value: &str) -> Result<u32, String> {
    match value.parse::<u32>() {
        Ok(mib @ (1 | 2 | 4 | 8 | 16 | 32 | 64)) => Ok(mib),
        _ => Err(format!(
            "{VARIABLE} must be one of 1, 2, 4, 8, 16, 32, 64 MiB."
        )),
    }
}

pub(crate) fn configure_from_environment() -> Result<(), String> {
    match env::var(VARIABLE) {
        Ok(value) => {
            let mib = parse_mib(&value)?;
            // Set process-wide V8 flags before any isolate or platform is initialized.
            deno_core::v8::V8::set_flags_from_string(&format!("--max-semi-space-size={mib}"));
            eprintln!("{VARIABLE}={mib}");
            Ok(())
        }
        Err(env::VarError::NotPresent) => Ok(()),
        Err(error) => Err(format!("Cannot read {VARIABLE}: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_mib;

    #[test]
    fn accepts_supported_semi_space_sizes() {
        for mib in [1, 2, 4, 8, 16, 32, 64] {
            assert_eq!(parse_mib(&mib.to_string()).unwrap(), mib);
        }
    }

    #[test]
    fn rejects_invalid_and_unsupported_sizes() {
        for value in ["", "0", "-1", "3", "65", "128", "1.5", "abc", "4294967296"] {
            assert!(parse_mib(value).is_err(), "Accepted invalid size: {value}");
        }
    }
}
