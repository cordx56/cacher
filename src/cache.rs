use std::collections::HashSet;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};
use tokio::{
    runtime::{Builder, Handle, Runtime},
    task::JoinHandle,
};

static MIR_CACHE: LazyLock<RwLock<HashSet<String>>> = LazyLock::new(|| RwLock::new(HashSet::new()));
static MIR_CACHE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let cache_dir = crate::cargo::get_cacher_target_dir();
    let crate_name = env::var("CARGO_CRATE_NAME").unwrap();
    let file_name = format!("{}.mir", crate_name.trim());
    cache_dir.join(file_name)
});
pub static RUNTIME: LazyLock<RwLock<Runtime>> =
    LazyLock::new(|| RwLock::new(Builder::new_multi_thread().enable_all().build().unwrap()));
pub static HANDLE: LazyLock<Handle> = LazyLock::new(|| RUNTIME.read().unwrap().handle().clone());

pub fn setup_cache() -> JoinHandle<()> {
    HANDLE.spawn(async move {
        if let Ok(mut f) = File::open(&*MIR_CACHE_PATH) {
            let mut buf = Vec::with_capacity(1_000_000);
            f.read_to_end(&mut buf).unwrap();
            *MIR_CACHE.write().unwrap() = serde_json::from_slice(&buf).unwrap_or(HashSet::new());
            log::info!("cache file read");
        }
    })
}
pub fn is_cached(mir: &str) -> bool {
    MIR_CACHE.read().unwrap().contains(mir)
}
pub fn add_cache(mir: String) {
    MIR_CACHE.write().unwrap().insert(mir);
}
pub fn save_cache() -> JoinHandle<()> {
    HANDLE.spawn(async move {
        if let Ok(mut f) = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*MIR_CACHE_PATH)
        {
            f.write_all(&serde_json::to_vec(&*MIR_CACHE.read().unwrap()).unwrap())
                .unwrap();
        }
    })
}
