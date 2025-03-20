use std::env;
use std::process;

fn main() {
    let mut cmd = process::Command::new(env::var("CARGO").unwrap());
    cmd.args(env::args().skip(2))
        .env("RUSTC", "fustc")
        .env("FUSTC_TARGET_DIR", fustc_utils::get_fustc_target_dir());
    let code = cmd.spawn().unwrap().wait().unwrap().code().unwrap();
    process::exit(code);
}
