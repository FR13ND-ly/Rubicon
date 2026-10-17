use std::{cell::RefCell, rc::Rc};

use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use deno_core::{OpState, ToJsBuffer, op2, serde_v8::ByteString};
use deno_error::JsErrorBox;
use serde::{Deserialize, Serialize};

use crate::Backend;

#[derive(Deserialize)]
struct BackendRequest {
    method: String,
    url: String,
    headers: Vec<(String, ByteString)>,
}

#[derive(Serialize)]
struct BackendResponse {
    status: u16,
    headers: Vec<(String, ByteString)>,
    body: ToJsBuffer,
}

#[derive(Clone)]
struct BackendState {
    backend: Backend,
    max_response_bytes: usize,
}

#[op2]
#[serde]
async fn op_rubicon_backend_request(
    state: Rc<RefCell<OpState>>,
    #[serde] request: BackendRequest,
    #[buffer(copy)] body: Vec<u8>,
) -> Result<BackendResponse, JsErrorBox> {
    let backend = state.borrow().borrow::<BackendState>().clone();

    let mut builder = Request::builder()
        .method(request.method.as_str())
        .uri(request.url);

    for (name, value) in request.headers {
        builder = builder.header(name, value.as_ref());
    }

    let request = builder
        .body(Body::from(body))
        .map_err(|error| JsErrorBox::type_error(error.to_string()))?;

    let response = backend.backend.dispatch(request).await;
    let (parts, body) = response.into_parts();

    let body = to_bytes(body, backend.max_response_bytes)
        .await
        .map_err(|error| JsErrorBox::generic(error.to_string()))?;

    let headers = parts
        .headers
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), ByteString::from(value.as_bytes())))
        .collect();

    Ok(BackendResponse {
        status: parts.status.as_u16(),
        headers,
        body: body.to_vec().into(),
    })
}

deno_core::extension!(
    rubicon_backend,
    ops = [op_rubicon_backend_request],
    options = {
        backend: Backend,
        max_response_bytes: usize,
    },
    state = |state, options| {
        state.put(BackendState {
            backend: options.backend,
            max_response_bytes: options.max_response_bytes,
        });
    },
);

pub fn backend_extension(backend: Backend, max_response_bytes: usize) -> deno_core::Extension {
    rubicon_backend::init(backend, max_response_bytes)
}
