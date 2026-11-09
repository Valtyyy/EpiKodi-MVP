use crate::smb::{self, Connexions};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderName, Method, StatusCode},
    response::Response,
    routing::{on, MethodFilter},
    Router,
};
use bytes::Bytes;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use ::smb::{GetLen, ReadAt};
use std::{io, sync::Arc};

const CHUNK: u64 = 1024 * 1024;
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

struct Shared {
    connexions: Connexions,
    token: String,
}

pub struct Proxy {
    port: u16,
    token: String,
}

impl Proxy {
    pub async fn start(connexions: Connexions) -> Result<Proxy, String> {
        let token = format!("{:032x}", rand::random::<u128>());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("Proxy local impossible : {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let app = Router::new()
            .route(
                "/{token}/{conn}/{*path}",
                on(MethodFilter::GET.or(MethodFilter::HEAD), serve),
            )
            .with_state(Arc::new(Shared {
                connexions,
                token: token.clone(),
            }));
        tauri::async_runtime::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok(Proxy { port, token })
    }

    pub fn url(&self, conn: &str, path: &str) -> String {
        let path = path
            .trim_matches('/')
            .split('/')
            .map(|s| utf8_percent_encode(s, SEGMENT).to_string())
            .collect::<Vec<_>>()
            .join("/");
        format!("http://127.0.0.1:{}/{}/{conn}/{path}", self.port, self.token)
    }
}

fn reply(status: StatusCode, headers: &[(HeaderName, String)], body: Body) -> Response {
    let mut builder = Response::builder().status(status);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    builder.body(body).unwrap()
}

fn parse_range(value: &str, len: u64) -> Option<(u64, u64)> {
    let last = len.checked_sub(1)?;
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    let (start, end) = match (start, end) {
        ("", n) => (len.checked_sub(n.parse().ok()?)?, last),
        (a, "") => (a.parse().ok()?, last),
        (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(last)),
    };
    (start <= end && end < len).then_some((start, end))
}

async fn serve(
    State(shared): State<Arc<Shared>>,
    method: Method,
    Path((token, conn, path)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    let text = |status: StatusCode, msg: &str| reply(status, &[], Body::from(msg.to_string()));
    if token != shared.token {
        return text(StatusCode::FORBIDDEN, "Forbidden");
    }
    let file = match smb::open_file(&shared.connexions, &conn, &path).await {
        Ok(f) => f,
        Err(e) => return text(StatusCode::NOT_FOUND, &e),
    };
    let len = match file.get_len().await {
        Ok(l) => l,
        Err(e) => return text(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let mut out = vec![
        (header::ACCEPT_RANGES, "bytes".to_string()),
        (header::CONTENT_TYPE, "application/octet-stream".to_string()),
    ];
    let (status, start, end) = match headers.get(header::RANGE) {
        None => (StatusCode::OK, 0, len),
        Some(value) => match value.to_str().ok().and_then(|v| parse_range(v, len)) {
            Some((a, b)) => {
                out.push((header::CONTENT_RANGE, format!("bytes {a}-{b}/{len}")));
                (StatusCode::PARTIAL_CONTENT, a, b + 1)
            }
            None => {
                out.push((header::CONTENT_RANGE, format!("bytes */{len}")));
                return reply(StatusCode::RANGE_NOT_SATISFIABLE, &out, Body::empty());
            }
        },
    };
    out.push((header::CONTENT_LENGTH, (end - start).to_string()));

    if method == Method::HEAD {
        return reply(status, &out, Body::empty());
    }
    // Le fichier SMB appartient au flux : si le client coupe, hyper abandonne le corps et le
    // Drop du handle lance un `close` côté serveur (tokio::spawn dans ResourceHandle::drop).
    let stream = futures_util::stream::unfold((file, start), move |(file, pos)| async move {
        if pos >= end {
            return None;
        }
        let mut buf = vec![0u8; (end - pos).min(CHUNK) as usize];
        match file.read_at(&mut buf, pos).await {
            Ok(0) => Some((Err(io::Error::from(io::ErrorKind::UnexpectedEof)), (file, end))),
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(Bytes::from(buf)), (file, pos + n as u64)))
            }
            Err(e) => Some((Err(io::Error::other(e.to_string())), (file, end))),
        }
    });
    reply(status, &out, Body::from_stream(stream))
}
