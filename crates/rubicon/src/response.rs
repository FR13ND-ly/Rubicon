use serde::{Deserialize, Deserializer, de::Visitor};
use std::{fmt, str::Utf8Error};

/// Buffered response data owned by Rust. Binary data is copied out of V8 on
/// the renderer thread, so no V8 backing store crosses into the HTTP task.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderBody {
    Text(String),
    Bytes(Vec<u8>),
}

impl RenderBody {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Text(text) => text.as_bytes(),
            Self::Bytes(bytes) => bytes,
        }
    }

    pub fn as_text(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }
}

impl From<String> for RenderBody {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for RenderBody {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<Vec<u8>> for RenderBody {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

impl PartialEq<&str> for RenderBody {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<String> for RenderBody {
    fn eq(&self, other: &String) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl<'de> Deserialize<'de> for RenderBody {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BodyVisitor;

        impl<'de> Visitor<'de> for BodyVisitor {
            type Value = RenderBody;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string, ArrayBuffer, or ArrayBuffer view")
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(RenderBody::Text(value))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(value.into())
            }

            fn visit_byte_buf<E: serde::de::Error>(self, value: Vec<u8>) -> Result<Self::Value, E> {
                Ok(RenderBody::Bytes(value))
            }

            fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
                Ok(RenderBody::Bytes(value.to_vec()))
            }
        }

        // serde_v8 copies ArrayBuffer/View data into an owned Vec before this
        // visitor receives it. Plain JS arrays and objects remain invalid.
        deserializer.deserialize_any(BodyVisitor)
    }
}

/// A buffered body or a bounded stream of Rust-owned chunks.
#[derive(Debug)]
pub enum RenderContent {
    Buffered(RenderBody),
    Stream(crate::streaming::RenderStream),
}

#[derive(Debug, Deserialize)]
pub struct RenderResponse<B = RenderBody> {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: B,
}
