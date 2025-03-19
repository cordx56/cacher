#![feature(rustc_private)]

pub extern crate indexmap;
pub extern crate polonius_engine;
pub extern crate rustc_borrowck;
pub extern crate rustc_data_structures;
pub extern crate rustc_driver;
pub extern crate rustc_errors;
pub extern crate rustc_hash;
pub extern crate rustc_hir;
pub extern crate rustc_hir_pretty;
pub extern crate rustc_hir_typeck;
pub extern crate rustc_index;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_session;
pub extern crate rustc_span;
pub extern crate smallvec;

mod analyzer;

use fustc::{models::*, tcp};
use rustc_data_structures::steal::Steal;
use rustc_driver::{Callbacks, Compilation, run_compiler};
use rustc_hir::{def_id::LocalDefId, hir_id::OwnerId};
use rustc_interface::interface;
use rustc_middle::{
    mir::{
        Body, BorrowCheckResult,
        pretty::{PrettyPrintMirOptions, write_mir_fn},
    },
    query::queries,
    ty::{TyCtxt, TypeckResults},
    util::Providers,
};
use rustc_session::{EarlyDiagCtxt, config};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, RwLock, atomic::AtomicBool};
use tokio::{
    //fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    runtime::{Builder, Handle, Runtime},
    //task::JoinSet,
};

use rustc_hir::{hir_id::HirId, intravisit::Visitor};

/*
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = tikv_jemallocator::Jemalloc;
*/

/*
static MIR_CACHE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    env::var("FUSTC_CWD")
        .map(|v| PathBuf::from(v))
        .unwrap_or(env::current_dir().unwrap())
        .join(".mirs")
});
*/
/*
static TASKS: LazyLock<RwLock<JoinSet<()>>> = LazyLock::new(|| RwLock::new(JoinSet::new()));
static MIR_CACHE: LazyLock<RwLock<HashSet<&'static str>>> =
    //LazyLock::new(|| RwLock::new(read_cache().unwrap_or(HashSet::new())));
    LazyLock::new(|| RwLock::new(HashSet::new()));
*/
static TCP_STREAM: LazyLock<Mutex<TcpStream>> =
    LazyLock::new(|| Mutex::new(TcpStream::connect("127.0.0.1:9081").unwrap()));

/*
thread_local! {
    static STAT_LOG: RefCell<Box<dyn Fn(&str)>> = RefCell::new(
        if env::var("FUSTC_STAT").map(|v| 0 < v.len()).unwrap_or(false) {
            Box::new(|message: &str| {
                use std::time::{SystemTime, UNIX_EPOCH};
                let now = SystemTime::now();
                let nanos = now.duration_since(UNIX_EPOCH).unwrap().as_nanos();
                println!("{nanos},{message}");
            })
        } else {
            Box::new(|_mes| {})
        },
    );
}
    */

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);

pub struct RustcCallback;
impl Callbacks for RustcCallback {}

/*
fn open_memfile() -> Option<memfile::MemFile> {
    use std::io::{Seek, SeekFrom};
    if let Ok(Ok(raw_fd)) = env::var("MIR_CACHE_FD").map(|v| v.parse()) {
        if let Ok(mut mfile) = unsafe { memfile::MemFile::from_fd(FromRawFd::from_raw_fd(raw_fd)) }
        {
            mfile.seek(SeekFrom::Start(0)).unwrap();
            return Some(mfile);
        }
    }
    log::warn!("no cache FD");
    None
}
fn cache_str_to_map(s: &'static str) -> HashSet<&'static str> {
    s.split("\n\n\n\n\n").map(|v| v.trim()).collect()
}
fn read_cache() -> Option<HashSet<&'static str>> {
    if let Some(mut mfile) = open_memfile() {
        let mut cache_vec = Vec::with_capacity(1024);
        let size = mfile.read_to_end(&mut cache_vec).unwrap();
        println!("{size}");
        let cache_string = unsafe { String::from_utf8_unchecked(cache_vec) };
        let cache_str: &'static str = unsafe { std::mem::transmute(cache_string.as_str()) };
        let map = cache_str_to_map(cache_str);
        std::mem::forget(cache_string);
        log::info!("cache size: {}", map.len());
        return Some(map);
    }
    log::warn!("failed to read memfile");
    None
}
*/

fn send_tcp(payload: &[u8]) {
    let mut stream = TCP_STREAM.lock().unwrap();
    stream.write_all(payload).unwrap();
    stream.flush().unwrap();
}
fn recv_tcp() -> Vec<u8> {
    let mut stream = TCP_STREAM.lock().unwrap();
    let mut buf = Vec::with_capacity(1024);
    loop {
        let mut read = [0; 1024];
        let len = stream.read(&mut read).unwrap();
        buf.extend_from_slice(&read[..len]);
        if len < 1024 {
            break;
        }
    }
    buf
}
fn request(req: FustcRequest) -> Option<WrapperResponse> {
    let mut stream = TCP_STREAM.lock().unwrap();
    let payload = serde_json::to_vec(&req).unwrap();
    stream
        .write_all(b"POST / HTTP/1.1\r\ncontent-length:")
        .unwrap();
    stream
        .write_all(payload.len().to_string().as_bytes())
        .unwrap();
    stream.write_all(b"\r\n\r\n").unwrap();
    stream.write_all(&payload).unwrap();
    stream.flush().unwrap();
    match req {
        FustcRequest::CacheSave { .. } => None,
        _ => {
            let mut buf = Vec::with_capacity(1024);
            let resp = tcp::read_stream_sync(&mut buf, &mut *stream);
            resp
        }
    }
}

static MIR_CACHE: LazyLock<RwLock<HashSet<String>>> = LazyLock::new(|| RwLock::new(HashSet::new()));
static MIR_CACHE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    let target_dir = PathBuf::from(env::var("CARGO_TARGET_DIR").unwrap().trim());
    let cache_dir = target_dir.join("fustc");
    create_dir_all(&cache_dir).unwrap();
    let crate_name = env::var("CARGO_CRATE_NAME").unwrap();
    let file_name = format!("{}.mir", crate_name.trim());
    cache_dir.join(file_name)
});
static RUNTIME: LazyLock<RwLock<Runtime>> =
    LazyLock::new(|| RwLock::new(Builder::new_multi_thread().enable_all().build().unwrap()));
static HANDLE: LazyLock<Handle> = LazyLock::new(|| RUNTIME.read().unwrap().handle().clone());

fn setup_cache<'tcx>() {
    HANDLE.spawn(async move {
        if let Ok(mut f) = File::open(&*MIR_CACHE_PATH) {
            let mut buf = Vec::with_capacity(1000_000);
            f.read_to_end(&mut buf).unwrap();
            *MIR_CACHE.write().unwrap() = serde_json::from_slice(&buf).unwrap_or(HashSet::new());
        }
    });
}
fn is_cached(mir: &str) -> bool {
    MIR_CACHE.read().unwrap().contains(mir)
}
fn add_cache(mir: String) {
    MIR_CACHE.write().unwrap().insert(mir);
}
fn save_cache() {
    if let Ok(mut f) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&*MIR_CACHE_PATH)
    {
        f.write_all(&serde_json::to_vec(&*MIR_CACHE.read().unwrap()).unwrap())
            .unwrap();
    }
}

#[inline]
fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    //local.analysis = analysis;
    local.mir_borrowck = mir_borrowck;
    //local.check_liveness = check_liveness;
    //local.typeck = typeck;
    //local.mir_built = mir_built;
}
/*
fn mir_built<'tcx>(tcx: TyCtxt<'tcx>, id: LocalDefId) -> &Steal<Body<'tcx>> {
    let hir_id = tcx.local_def_id_to_hir_id(id);
    let pretty = rustc_hir_pretty::id_to_string(&tcx.hir(), hir_id);
}
*/
#[allow(unused)]
fn typeck<'tcx>(
    tcx: TyCtxt<'tcx>,
    key: queries::typeck::LocalKey<'tcx>,
) -> queries::typeck::ProvidedValue<'tcx> {
    let mut visitor = analyzer::HirFnVisitor::new(tcx);
    let node = tcx.hir_node_by_def_id(key);
    if let Some(body_id) = node.body_id() {
        visitor.visit_nested_body(body_id);
    }
    let mut providers = Providers::default();
    rustc_hir_typeck::provide(&mut providers);
    (providers.typeck)(tcx, key)
    //let results = TypeckResults::new(OwnerId { def_id: key });

    //tcx.arena.alloc(results)
}
#[allow(unused)]
fn analysis<'tcx>(
    _tcx: TyCtxt<'tcx>,
    _key: queries::analysis::LocalKey,
) -> queries::analysis::ProvidedValue<'tcx> {
}
#[inline]
fn default_mir_borrowck<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
) -> queries::mir_borrowck::ProvidedValue<'tcx> {
    let mut providers = Providers::default();
    rustc_borrowck::provide(&mut providers);
    (providers.mir_borrowck)(tcx, def_id)
}
#[inline]
fn mir_borrowck<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
) -> queries::mir_borrowck::ProvidedValue<'tcx> {
    //let key = tcx.def_path_str(def_id.to_def_id());

    //log::info!("start borrowck of {def_id:?}");
    //STAT_LOG.with(|f| f.borrow()(&format!("{key},start")));

    /*
    use std::time::SystemTime;
    let borrowck_start = SystemTime::now();
    let mut tcpio_start = 0;
    */

    if tcx.hir_body_const_context(def_id.to_def_id()).is_some() {
        let result = default_mir_borrowck(tcx, def_id);
        //STAT_LOG.with(|f| f.borrow()(&format!("{key},no_cache")));
        return result;
    }

    let empty_result = BorrowCheckResult {
        concrete_opaque_types: indexmap::IndexMap::default(),
        closure_requirements: None,
        used_mut_upvars: smallvec::SmallVec::new(),
        tainted_by_errors: None,
    };

    let mut compiling_mir = Vec::with_capacity(1000_000);
    let mut compiling_mir_string = String::new();
    let body = tcx.mir_built(def_id);
    if !body.is_stolen() {
        write_mir_fn(
            tcx,
            &*body.borrow(),
            &mut |_, _| Ok(()),
            &mut compiling_mir,
            PrettyPrintMirOptions {
                include_extra_comments: false,
            },
        )
        .unwrap();

        if 0 < compiling_mir.len() {
            compiling_mir_string = unsafe { String::from_utf8_unchecked(compiling_mir) };

            //let start = SystemTime::now();
            //let mut conn = TcpStream::connect("127.0.0.1:9081").unwrap();

            /*
            let resp = request(FustcRequest::CacheCheck {
                mir: compiling_mir_string.clone(),
            })
            .unwrap();
            */

            //log::info!("{resp:?}");
            /*
            conn.write_all(req.as_bytes()).unwrap();
            conn.shutdown(Shutdown::Write).unwrap();
            let mut buf = Vec::with_capacity(1024);
            conn.read_to_end(&mut buf).unwrap();
            */
            //tcpio_start += SystemTime::now().duration_since(start).unwrap().as_nanos();
            /*
            tcpio_start += SystemTime::now()
                .duration_since(start)
                .unwrap_or_else(|v| v.duration())
                .as_nanos();
            */

            //if MIR_CACHE.read().unwrap().contains(&compiling_mir_string) {
            if is_cached(&compiling_mir_string) {
                //if let WrapperResponse::CacheStatus { cached: true } = resp {
                log::debug!("{def_id:?} cache hit");

                /*
                println!(
                    "{}",
                    serde_json::to_string(&fustc::Metrics::TcpIo(tcpio_start.to_string())).unwrap()
                );

                let dur = SystemTime::now()
                    .duration_since(borrowck_start)
                    .unwrap_or_else(|v| v.duration());
                println!(
                    "{}",
                    serde_json::to_string(&fustc::Metrics::Borrowck(dur.as_nanos().to_string()))
                        .unwrap()
                );
                */

                return tcx.arena.alloc(empty_result);
            }
            /*
            let compiling_mir_str: &'static str =
                unsafe { std::mem::transmute(compiling_mir_string.trim()) };
            if let Ok(cache) = MIR_CACHE.read() {
                if cache.contains(&compiling_mir_str) {
                    log::debug!("{def_id:?} cache hit");
                    //STAT_LOG.with(|f| f.borrow()(&format!("{key},cache_hit")));
                    return tcx.arena.alloc(empty_result);
                }
            }
            */
        }
    }

    log::debug!("{def_id:?} no cache; start mir_borrowck");

    let result = default_mir_borrowck(tcx, def_id);
    let can_cache = result.concrete_opaque_types.is_empty()
        && result.closure_requirements.is_none()
        && result.used_mut_upvars.is_empty()
        && result.tainted_by_errors.is_none()
        && 0 < compiling_mir_string.len();
    if can_cache {
        //let start = SystemTime::now();
        add_cache(compiling_mir_string);
        log::info!("{def_id:?} cache saved");
        /*
        RUNTIME.write().unwrap().spawn(async {
            //let mut conn = TcpStream::connect("127.0.0.1:9081").unwrap();

            request(FustcRequest::CacheSave {
                mir: compiling_mir_string,
            });
            log::info!("cache save");
            //conn.write_all(&req).unwrap();
        });
        */
        /*
        tcpio_start += SystemTime::now()
            .duration_since(start)
            .unwrap_or_else(|v| v.duration())
            .as_nanos();
        */
        /*
        let compiling_mir_str: &'static str =
            unsafe { std::mem::transmute(compiling_mir_string.trim()) };
        if let Ok(mut cache) = MIR_CACHE.write() {
            cache.insert(&compiling_mir_str);
            //STAT_LOG.with(|f| f.borrow()(&format!("{key},cached")));
            log::debug!("{def_id:?} cached");
        }
        */
    } else {
        log::debug!("{def_id:?} cannot be cached due to its mir_borrowck result")
    }
    //std::mem::forget(compiling_mir_string);
    //STAT_LOG.with(|f| f.borrow()(&format!("{def_id:?},no_cache")));

    /*
    println!(
        "{}",
        serde_json::to_string(&fustc::Metrics::TcpIo(tcpio_start.to_string())).unwrap()
    );
    let dur = SystemTime::now()
        .duration_since(borrowck_start)
        .unwrap_or_else(|v| v.duration())
        .as_nanos();
    println!(
        "{}",
        serde_json::to_string(&fustc::Metrics::Borrowck(dur.to_string())).unwrap()
    );
    log::info!("borrowck finished");
    */

    result
}
#[allow(unused)]
fn check_liveness<'tcx>(_tcx: TyCtxt<'tcx>, _def_id: LocalDefId) {}

pub struct FustcCallback;
impl Callbacks for FustcCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.using_internal_features = &ATOMIC_TRUE;
        config.override_queries = Some(override_queries);

        setup_cache();

        /*
        if let Ok(mut file) = OpenOptions::new()
            .read(true)
            //.append(true)
            .open(PathBuf::from(env::var("FUSTC_CWD").unwrap()).join(".mirs"))
        //.await
        {
            let mut cache_string = String::new();
            file.read_to_string(&mut cache_string).unwrap();
            let cache: HashSet<_> = cache_string.split("\n\n\n\n\n").map(|v| v.trim()).collect();
            unsafe {
                *MIR_CACHE.write().unwrap() = std::mem::transmute(cache);
            }
            std::mem::forget(cache_string);
        } else {
            log::warn!("cache file open failed");
        }

        TASKS.write().unwrap().spawn_on(
            async {
                let json_path = MIR_JSON_PATH.with(|json_path| json_path.clone());
                if let Ok(mut file) = File::open(json_path).await {
                    let mut json = Vec::with_capacity(1024);
                    file.read_to_end(&mut json).await.ok();
                    if let Ok(mut cache) = MIR_CACHE.write() {
                        if let Ok(data) = serde_json::from_slice(&json) {
                            *cache = data;
                        }
                    }
                }
            },
            &HANDLE,
        );
        */
    }
    /*
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        HANDLE.block_on(async {
            while let Some(_) = { TASKS.write().unwrap().join_next().await } {}
        });
        Compilation::Continue
    }
    */
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        save_cache();
        Compilation::Continue
    }
    /*
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create(true)
            //.append(true)
            .truncate(true)
            .open(PathBuf::from(env::var("FUSTC_CWD").unwrap()).join(".mirs"))
        //.await
        {
            if let Ok(cache) = MIR_CACHE.read() {
                for iter in cache.iter() {
                    file.write_all((*iter).as_bytes()).unwrap();
                    file.write_all(b"\n\n\n\n\n").unwrap();
                }
                return Compilation::Continue;
            }
            log::info!("cache file written");
        }
        log::warn!("write cache file failed");
        Compilation::Continue
    }
    */
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compiler {
    Normal,
    Fast,
}

pub fn run_fustc(compiler: Compiler) -> i32 {
    let ctxt = EarlyDiagCtxt::new(config::ErrorOutputType::default());
    let args = rustc_driver::args::raw_args(&ctxt);
    let args = &args[1..];

    if compiler == Compiler::Normal {
        return rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut RustcCallback));
    }
    for arg in args {
        if arg == "-vV" || arg == "--version" || arg.starts_with("--print") {
            return rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut RustcCallback));
        }
    }

    /*
    let mut conn = connect();
    let req = serde_json::to_string(&FustcRequest::GetCache).unwrap();
    conn.write_all(req.as_bytes()).unwrap();
    conn.shutdown(Shutdown::Write).unwrap();
    let mut buf = Vec::with_capacity(1000_000);
    conn.read_to_end(&mut buf).unwrap();
    *MIR_CACHE.write().unwrap() = serde_json::from_slice(&buf).unwrap();
    log::info!("cache obtained");
    */

    rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut FustcCallback))
}

fn main() {
    {
        use std::os::raw::{c_int, c_void};

        use tikv_jemalloc_sys as jemalloc_sys;

        #[used]
        static _F1: unsafe extern "C" fn(usize, usize) -> *mut c_void = jemalloc_sys::calloc;
        #[used]
        static _F2: unsafe extern "C" fn(*mut *mut c_void, usize, usize) -> c_int =
            jemalloc_sys::posix_memalign;
        #[used]
        static _F3: unsafe extern "C" fn(usize, usize) -> *mut c_void = jemalloc_sys::aligned_alloc;
        #[used]
        static _F4: unsafe extern "C" fn(usize) -> *mut c_void = jemalloc_sys::malloc;
        #[used]
        static _F5: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void = jemalloc_sys::realloc;
        #[used]
        static _F6: unsafe extern "C" fn(*mut c_void) = jemalloc_sys::free;

        // On OSX, jemalloc doesn't directly override malloc/free, but instead
        // registers itself with the allocator's zone APIs in a ctor. However,
        // the linker doesn't seem to consider ctors as "used" when statically
        // linking, so we need to explicitly depend on the function.
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn _rjem_je_zone_register();
            }

            #[used]
            static _F7: unsafe extern "C" fn() = _rjem_je_zone_register;
        }
    }

    simple_logger::SimpleLogger::new()
        .with_level(env::var("FUSTC_LOG").map_or(log::LevelFilter::Error, |v| {
            v.parse().unwrap_or(log::LevelFilter::Error)
        }))
        .with_colors(true)
        .init()
        .unwrap();

    let fast_result = std::panic::catch_unwind(|| run_fustc(Compiler::Fast));
    let code = match fast_result {
        Ok(0) => 0,
        _ => {
            log::error!("fallback normal rustc");
            run_fustc(Compiler::Normal)
        }
    };

    //HANDLE.block_on(async { while let Some(_) = TASKS.lock().unwrap().join_next().await {} });

    std::process::exit(code);
}
