use httparse::{EMPTY_HEADER, Request, Response};
use serde::Deserialize;
use std::io::Read;
use tokio::io::{AsyncRead, AsyncReadExt};

fn parse<T>(buf: &[u8]) -> (Option<T>, &[u8])
where
    T: for<'de> Deserialize<'de> + 'static,
{
    let mut headers = [EMPTY_HEADER; 5];
    log::info!("{}", String::from_utf8_lossy(buf));
    let res = if buf.starts_with(b"HTTP") {
        let mut parser = Response::new(&mut headers);
        let res = parser.parse(buf).unwrap();
        res
    } else {
        let mut parser = Request::new(&mut headers);
        let res = parser.parse(buf).unwrap();
        res
    };
    if !res.is_partial() {
        log::warn!("{}", String::from_utf8_lossy(buf));
        let from = res.unwrap();
        for h in headers {
            if h.name.to_lowercase() == "content-length" {
                let len: usize = String::from_utf8_lossy(h.value).parse().unwrap();
                if from + len <= buf.len() {
                    if let Ok(payload) = serde_json::from_slice(&buf[from..from + len]) {
                        return (Some(payload), &buf[from + len..]);
                    }
                }
            }
        }
    }
    (None, buf)
}

#[inline]
pub fn read_stream_sync<T, R>(buf: &mut Vec<u8>, stream: &mut T) -> Option<R>
where
    T: Read + Unpin,
    R: for<'de> Deserialize<'de> + 'static,
{
    loop {
        let mut read = [0; 1024];
        let len = stream.read(&mut read).unwrap_or(0);
        buf.extend_from_slice(&read[..len]);
        match parse(&buf) {
            (Some(res), remain) => {
                *buf = remain.to_vec();
                return Some(res);
            }
            _ => {
                if len == 0 {
                    return None;
                }
            }
        }
    }
}

#[inline]
pub async fn read_stream_async<T, R>(buf: &mut Vec<u8>, stream: &mut T) -> Option<R>
where
    T: AsyncRead + Unpin,
    R: for<'de> Deserialize<'de> + 'static,
{
    loop {
        let mut read = [0; 1024];
        let len = stream.read(&mut read).await.unwrap_or(0);
        buf.extend_from_slice(&read[..len]);
        match parse(&buf) {
            (Some(res), remain) => {
                *buf = remain.to_vec();
                return Some(res);
            }
            _ => {
                if len == 0 {
                    return None;
                }
            }
        }
    }
}
