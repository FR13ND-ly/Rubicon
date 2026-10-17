use rubicon::JavaScriptRuntime;

const VARIABLE: &str = "RUBICON_DEMO_BODY_TRANSPORT";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BodyTransport {
    Text,
    Bytes,
    Stream,
}

impl BodyTransport {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "bytes" => Ok(Self::Bytes),
            "stream" => Ok(Self::Stream),
            _ => Err(format!("{VARIABLE} must be text, bytes, or stream.")),
        }
    }

    pub(crate) fn from_environment() -> Result<Self, String> {
        let transport = match std::env::var(VARIABLE) {
            Ok(value) => Self::parse(&value)?,
            Err(std::env::VarError::NotPresent) => Self::Text,
            Err(error) => return Err(format!("Cannot read {VARIABLE}: {error}")),
        };
        if transport == Self::Stream && !cfg!(feature = "streaming") {
            return Err(
                "Stream transport requires a server built with --features streaming.".to_owned(),
            );
        }
        eprintln!("{VARIABLE}={}", transport.name());
        Ok(transport)
    }

    fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Bytes => "bytes",
            Self::Stream => "stream",
        }
    }

    pub(crate) fn configure(self, runtime: &mut JavaScriptRuntime) -> Result<(), String> {
        runtime.execute_i64(&format!(
            "Object.defineProperty(globalThis, '__rubiconBodyTransport', {{ value: '{}' }}); 0;",
            self.name(),
        ))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BodyTransport;

    #[test]
    fn validates_transport_names() {
        assert_eq!(BodyTransport::parse("text").unwrap(), BodyTransport::Text);
        assert_eq!(BodyTransport::parse("bytes").unwrap(), BodyTransport::Bytes);
        assert_eq!(
            BodyTransport::parse("stream").unwrap(),
            BodyTransport::Stream
        );
        for value in ["", "byte", "BYTES", "bytes "] {
            assert!(BodyTransport::parse(value).is_err());
        }
    }
}
