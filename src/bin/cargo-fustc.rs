use fustc::server;
use std::env;
use std::path::PathBuf;
use tokio::process::Command;

#[tokio::main]
async fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Error)
        .env()
        .with_colors(true)
        .init()
        .unwrap();

    unsafe {
        env::set_var(
            "CARGO_TARGET_DIR",
            env::var("CARGO_TARGET_DIR")
                .map(|v| PathBuf::from(v))
                .unwrap_or(PathBuf::from(env::current_dir().unwrap()).join("target")),
        );
        env::set_var("RUSTC", "fustc");
    }
    let args = env::args().skip(2);

    /*
    tokio::spawn(async move {
        server::serve().await;
    });
    */

    let mut child = Command::new("cargo")
        .args(args)
        //.stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.wait().await.unwrap();

    /*
    use tokio::io::AsyncReadExt;
    let mut outstr = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut outstr)
        .await
        .unwrap();
    let metrics: Vec<fustc::Metrics> = outstr
        .split("\n")
        .filter_map(|v| serde_json::from_str(v).ok())
        .collect();
    let mut borrowck: u128 = 0;
    let mut tcpio: u128 = 0;
    for met in metrics {
        match met {
            fustc::Metrics::Borrowck(v) => borrowck += v.parse::<u128>().unwrap(),
            fustc::Metrics::TcpIo(v) => tcpio += v.parse::<u128>().unwrap(),
        }
    }
    println!("borrowck: {borrowck}ns, TcpIO: {tcpio}ns");
    */

    //server::save_cache().await;
}
