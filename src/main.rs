use memfile::MemFile;
use shared_memory::ShmemConf;
use std::collections::HashSet;
use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::{RwLock, Arc};

fn main() {
    if env::var("FUSTC_CWD").is_err() {
        unsafe {
            env::set_var("FUSTC_CWD", env::current_dir().unwrap());
        }
    }
    if let Some(cache) = read_cache() {
        let sm = ShmemConf::new()
            .size(std::mem::size_of::<RwLock<HashSet<String>>>())
            .create()
            .unwrap();
        let smptr = sm.as_ptr();
        unsafe {
            let ptr = smptr as *mut RwLock<HashSet<String>>;
            *ptr = RwLock::new(cache_str_to_map(&cache));
            env::set_var("MIR_CACHE", sm.get_os_id());
        }
        std::mem::forget(sm);
    }

    let rustup = CString::new("rustup").unwrap();
    let mut args = vec![
        rustup.clone(),
        CString::new("run").unwrap(),
        CString::new("nightly-2025-02-12").unwrap(),
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

fn cache_str_to_map(s: &str) -> HashSet<String> {
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
