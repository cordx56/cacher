use std::env;
use std::ffi::CString;

fn main() {
    let rustup = CString::new("rustup").unwrap();
    if env::var("FUSTC_CWD").is_err() {
        unsafe {
            env::set_var("FUSTC_CWD", env::current_dir().unwrap());
        }
    }
    let mut args = vec![
        rustup.clone(),
        CString::new("run").unwrap(),
        CString::new("nightly-2024-10-31").unwrap(),
        CString::new("cargo").unwrap(),
        CString::new("fustc_driver").unwrap(),
    ];
    args.extend(env::args().skip(2).map(|v| CString::new(v).unwrap()));
    nix::unistd::execvp(&rustup, &args).unwrap();
}
