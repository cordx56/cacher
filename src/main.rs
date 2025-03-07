use memfile::MemFile;
use std::collections::HashSet;
use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::PathBuf;

fn main() {
    if env::var("FUSTC_CWD").is_err() {
        unsafe {
            env::set_var("FUSTC_CWD", env::current_dir().unwrap());
        }
    }
    if let Some(cache) = read_cache() {
        let mut mfile = MemFile::create_default("MIR_CACHE").unwrap();
        mfile.write_all(cache.as_bytes()).unwrap();
        //mfile.flush().unwrap();
        nix::unistd::dup2(mfile.as_fd().as_raw_fd(), 5000);
        unsafe {
            env::set_var("MIR_CACHE_FD", 5000.to_string());
        }
    use std::io::{Seek, SeekFrom};
        //mfile.seek(SeekFrom::Start(0)).unwrap();
        std::mem::forget(mfile);
    }

    let rustup = CString::new("rustup").unwrap();
    let mut args = vec![
        rustup.clone(),
        CString::new("run").unwrap(),
        CString::new("nightly-2025-02-22").unwrap(),
        CString::new("cargo").unwrap(),
        CString::new("fustc_driver").unwrap(),
    ];
    args.extend(env::args().skip(2).map(|v| CString::new(v).unwrap()));
    //nix::unistd::execvp(&rustup, &args).unwrap();
    let mut args: Vec<_> = ["run", "nightly-2025-02-12", "cargo", "fustc_driver"]
        .into_iter()
        .map(|v| v.to_owned())
        .collect();
    args.extend(env::args().skip(2).collect::<Vec<_>>());
    let mut cmd = std::process::Command::new("rustup");
    cmd.args(args);
    cmd.spawn().unwrap().wait().unwrap();
}

fn cache_str_to_map(s: String) -> HashSet<String> {
    s.split("\n\n\n").map(|v| v.to_owned()).collect()
}
fn read_cache() -> Option<String> {
    if let Ok(mut file) = File::open(PathBuf::from(env::var("FUSTC_CWD").unwrap()).join(".mirs")) {
        log::warn!("no cache file");
        let mut cache_str = Vec::with_capacity(1024);
        file.read_to_end(&mut cache_str).ok();
        let cache = unsafe { String::from_utf8_unchecked(cache_str) };
        Some(cache)
    } else {
        None
    }
}
