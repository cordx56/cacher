use super::{models::*, tcp};
use std::collections::HashSet;
use std::sync::LazyLock;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpListener,
    sync::{Mutex, RwLock},
};

static MIR_CACHE: LazyLock<HashSet<String>> = LazyLock::new(|| {
    use std::fs::File;
    use std::io::Read;
    if let Ok(mut f) = File::open(".mirs") {
        let mut buf = Vec::with_capacity(1000_000);
        f.read_to_end(&mut buf).unwrap();
        if let Ok(json) = serde_json::from_slice(&buf) {
            return json;
        }
    }
    HashSet::new()
});
static NEW_CACHE: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub async fn save_cache() {
    use tokio::fs::OpenOptions;
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(".mirs")
        .await
        .unwrap();
    let mut lock = NEW_CACHE.lock().await;
    lock.extend(MIR_CACHE.clone());
    let content = serde_json::to_string(&*lock).unwrap();
    f.write_all(content.as_bytes()).await.unwrap();
}

pub async fn serve() {
    let listener = TcpListener::bind("127.0.0.1:9081").await.unwrap();

    loop {
        let (sock, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            sock_io(sock).await;
        });
    }
}

async fn sock_io<T>(mut sock: T)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    /*
    let mut buf = Vec::with_capacity(1000_000_000);
    sock.read_to_end(&mut buf).await.unwrap();
    */
    let mut buf = Vec::with_capacity(1024);
    loop {
        if let Some(req) = tcp::read_stream_async(&mut buf, &mut sock).await {
            match req {
                FustcRequest::GetCache => {
                    sock.write_all(&serde_json::to_vec(&*MIR_CACHE).unwrap())
                        .await
                        .unwrap();
                    sock.flush().await.unwrap();
                }
                FustcRequest::CacheCheck { mir } => {
                    let cached = MIR_CACHE.contains(&mir);
                    let payload =
                        serde_json::to_vec(&WrapperResponse::CacheStatus { cached }).unwrap();
                    sock.write_all(b"HTTP/1.1 200 OK\r\ncontent-length:").await.unwrap();
                    sock.write_all(payload.len().to_string().as_bytes())
                        .await
                        .unwrap();
                    sock.write_all(b"\r\n\r\n").await.unwrap();
                    sock.write_all(&payload).await.unwrap();
                    sock.flush().await.unwrap();
                }
                FustcRequest::CacheSave { mir } => {
                    NEW_CACHE.lock().await.insert(mir);
                }
            }
        } else {
            break;
        }
    }
}
