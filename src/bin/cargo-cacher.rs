use std::env;
use std::io::{BufRead, BufReader};
use std::process;

fn main() {
    let mut cmd = process::Command::new(env::var("CARGO").unwrap_or("cargo".to_owned()));
    cmd.args(env::args().skip(2))
        .env("RUSTC", "cacherc")
        .env("CACHER_TARGET_DIR", cacher::cargo::get_cacher_target_dir());
    if env::var("CACHER_STAT")
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        cmd.env("CACHER_LOG", "info")
            .stderr(process::Stdio::piped());
    }
    let mut child = cmd.spawn().unwrap();
    if let Some(stderr) = child.stderr.take() {
        let mut total = 0;
        let mut hit = 0;
        for line in BufReader::new(stderr).lines().flatten() {
            if line.find("no cache").is_some() {
                total += 1;
            } else if line.find("cache hit").is_some() {
                total += 1;
                hit += 1;
            }
        }
        println!("total: {total}, hit: {hit}, ratio: {}", hit * 100 / total);
    }
    let code = child.wait().unwrap().code().unwrap();

    process::exit(code);
}
