use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn get_cacher_target_dir() -> PathBuf {
    let cacher_dir = env::var("CACHER_TARGET_DIR")
        .ok()
        .map(PathBuf::from)
        .or(get_target_dir_from_cargo().map(|v| v.join("cacher")))
        .or(env::var("CARGO_TARGET_DIR")
            .ok()
            .map(|v| PathBuf::from(v).join("cacher")))
        .unwrap_or(env::current_dir().unwrap().join("target").join("cacher"));
    std::fs::create_dir_all(&cacher_dir).unwrap();
    cacher_dir
}

pub fn get_target_dir_from_cargo() -> Option<PathBuf> {
    let mut cargo = Command::new(env::var("CARGO").unwrap());
    cargo
        .args(["metadata", "--format-version=1", "--no-deps"])
        .stdout(Stdio::piped());
    let metadata = cargo.output().unwrap().stdout;
    serde_json::from_slice::<serde_json::Value>(&metadata)
        .ok()
        .and_then(|v| {
            v.get("target_directory")
                .map(|v| v.as_str().map(|v| PathBuf::from(v.trim())))
        })
        .flatten()
}
