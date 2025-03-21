use std::env;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn get_fustc_target_dir() -> PathBuf {
    let fustc_dir = env::var("FUSTC_TARGET_DIR")
        .ok()
        .map(PathBuf::from)
        .or(env::var("CARGO_TARGET_DIR")
            .ok()
            .map(|v| PathBuf::from(v).join("fustc")))
        .or(get_target_dir_from_cargo().map(|v| v.join("fustc")))
        .unwrap_or(env::current_dir().unwrap().join("target").join("fustc"));
    create_dir_all(&fustc_dir).unwrap();
    fustc_dir
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
