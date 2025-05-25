use std::env;
use std::process;

fn main() {
    let mut cmd = process::Command::new(env::var("CARGO").unwrap_or("cargo".to_owned()));
    cmd.args(env::args().skip(2))
        .env("RUSTC", "cacherc")
        .env("FUSTC_TARGET_DIR", cacher::cargo::get_fustc_target_dir());
    let code = cmd.spawn().unwrap().wait().unwrap().code().unwrap();
    process::exit(code);
}
