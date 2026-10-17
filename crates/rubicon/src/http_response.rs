use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
};

use crate::{RenderBody, RenderContent, RenderResponse};

fn checked_headers(
    status: u16,
    pairs: Vec<(String, String)>,
    empty: bool,
) -> Result<(StatusCode, HeaderMap), String> {
    if !(200..=599).contains(&status) {
        return Err("Response status must be between 200 and 599.".to_owned());
    }

    let status = StatusCode::from_u16(status)
        .map_err(|error| format!("Invalid response status: {error}"))?;

    if matches!(status.as_u16(), 204 | 205 | 304) && !empty {
        return Err("This response status requires an empty body.".to_owned());
    }

    let mut headers = HeaderMap::with_capacity(pairs.len());

    for (name, value) in pairs {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| format!("Invalid response header name: {error}"))?;

        if matches!(
            name.as_str(),
            "connection"
                | "content-length"
                | "transfer-encoding"
                | "keep-alive"
                | "proxy-connection"
                | "te"
                | "trailer"
                | "upgrade"
        ) {
            return Err(format!(
                "Response header is owned by the HTTP server: {name}"
            ));
        }

        let value = HeaderValue::try_from(value)
            .map_err(|error| format!("Invalid response header value: {error}"))?;

        headers.append(name, value);
    }

    Ok((status, headers))
}

pub fn into_http_response(rendered: RenderResponse) -> Result<Response, String> {
    let (status, headers) =
        checked_headers(rendered.status, rendered.headers, rendered.body.is_empty())?;
    let body = match rendered.body {
        RenderBody::Text(text) => Body::from(text),
        RenderBody::Bytes(bytes) => Body::from(bytes),
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    *response.headers_mut() = headers;

    Ok(response)
}

pub fn into_http_streaming_response(
    rendered: RenderResponse<RenderContent>,
) -> Result<Response, String> {
    match rendered.body {
        RenderContent::Buffered(body) => into_http_response(RenderResponse {
            status: rendered.status,
            headers: rendered.headers,
            body,
        }),
        RenderContent::Stream(stream) => {
            let (status, headers) = checked_headers(rendered.status, rendered.headers, false)?;
            let mut response = Response::new(Body::from_stream(stream));
            *response.status_mut() = status;
            *response.headers_mut() = headers;
            Ok(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, http::StatusCode};

    use super::into_http_response;
    use crate::RenderResponse;

    #[tokio::test]
    async fn preserves_redirect_cookies_and_utf8_body() {
        let body = "<p>Redirecționare</p>";
        let response = into_http_response(RenderResponse {
            status: 302,
            headers: vec![
                ("location".to_owned(), "/products".to_owned()),
                ("set-cookie".to_owned(), "first=1; Path=/".to_owned()),
                ("set-cookie".to_owned(), "second=2; Path=/".to_owned()),
            ],
            body: body.into(),
        })
        .expect("Response conversion failed");

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(response.headers()["location"], "/products");

        let cookies: Vec<&str> = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|value| value.to_str().expect("Cookie should be text"))
            .collect();

        assert_eq!(cookies, vec!["first=1; Path=/", "second=2; Path=/"]);

        let bytes = to_bytes(response.into_body(), 1024)
            .await
            .expect("Body reading failed");

        assert_eq!(bytes.as_ref(), body.as_bytes());
    }

    #[test]
    fn rejects_invalid_or_informational_statuses() {
        for status in [0, 99, 100, 199, 600, u16::MAX] {
            let result = into_http_response(RenderResponse {
                status,
                headers: Vec::new(),
                body: String::new().into(),
            });

            assert!(result.is_err(), "Status {status} should be rejected");
        }
    }

    #[test]
    fn rejects_invalid_headers_and_renderer_framing() {
        for (name, value) in [
            ("bad name", "value"),
            ("x-demo", "first\r\nInjected: second"),
            ("Content-Length", "999"),
            ("Transfer-Encoding", "chunked"),
            ("Connection", "close"),
        ] {
            let result = into_http_response(RenderResponse {
                status: 200,
                headers: vec![(name.to_owned(), value.to_owned())],
                body: "hello".into(),
            });

            assert!(result.is_err(), "Header {name} should be rejected");
        }
    }

    #[test]
    fn statuses_without_content_require_empty_bodies() {
        for status in [204, 205, 304] {
            assert!(
                into_http_response(RenderResponse {
                    status,
                    headers: Vec::new(),
                    body: "unexpected content".into(),
                })
                .is_err(),
            );

            assert!(
                into_http_response(RenderResponse {
                    status,
                    headers: Vec::new(),
                    body: String::new().into(),
                })
                .is_ok(),
            );
        }

        for status in [204, 205, 304] {
            assert!(
                into_http_response(RenderResponse {
                    status,
                    headers: Vec::new(),
                    body: vec![0].into(),
                })
                .is_err()
            );
            assert!(
                into_http_response(RenderResponse {
                    status,
                    headers: Vec::new(),
                    body: Vec::new().into(),
                })
                .is_ok()
            );
        }
    }
}
