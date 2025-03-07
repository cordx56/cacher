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
pub extern crate rustc_index;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_session;
pub extern crate rustc_span;
pub extern crate smallvec;

use rustc_data_structures::steal::Steal;
use rustc_driver::{run_compiler, Callbacks, Compilation};
use rustc_hir::{def_id::LocalDefId, hir_id::OwnerId};
use rustc_interface::interface;
use rustc_middle::{
    mir::{
        pretty::{write_mir_fn, PrettyPrintMirOptions},
        Body, BorrowCheckResult,
    },
    query::queries,
    ty::{TyCtxt, TypeckResults},
    util::Providers,
};
use rustc_session::{config, EarlyDiagCtxt};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, LazyLock, Mutex, RwLock};

thread_local! {
    static MIR_CACHE_PATH: PathBuf = env::var("FUSTC_CWD")
        .map(|v| PathBuf::from(v))
        .unwrap_or(env::current_dir().unwrap())
        .join(".mirs");
}
thread_local! {
static MIR_CACHE: *mut RwLock<HashSet<String>> ={
    let sm = shared_memory::ShmemConf::new()
        .os_id(&env::var("MIR_CACHE").unwrap())
        .open()
        .unwrap();
        let smptr = sm.as_ptr() as *mut RwLock<HashSet<String>>;
        smptr
    }
}

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

static ATOMIC_TRUE: AtomicBool = AtomicBool::new(true);

pub struct RustcCallback;
impl Callbacks for RustcCallback {}

fn open_memfile() -> Option<memfile::MemFile> {
    use std::io::{Seek, SeekFrom};
    if let Ok(Ok(raw_fd)) = env::var("MIR_CACHE_FD").map(|v| v.parse()) {
        if let Ok(mut mfile) =
            unsafe { memfile::MemFile::from_fd(std::os::fd::FromRawFd::from_raw_fd(raw_fd)) }
        {
            //mfile.seek(SeekFrom::Start(0)).unwrap();
            return Some(mfile);
        }
    }
    log::warn!("no cache FD");
    None
}
fn cache_str_to_map(s: String) -> HashSet<String> {
    s.split("\n\n\n").map(|v| v.trim().to_owned()).collect()
}
fn read_cache() -> Option<HashSet<String>> {
    if let Some(mut mfile) = open_memfile() {
        let mut cache_str = Vec::with_capacity(1024000);
        let size = mfile.read(&mut cache_str).unwrap();
        println!("{size}");
        let cache = unsafe { String::from_utf8_unchecked(cache_str[..size].to_vec()) };
        let map = cache_str_to_map(cache);
        log::info!("cache size: {}", map.len());
        return Some(map);
    }
    log::warn!("failed to read memfile");
    None
}

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
    let results = TypeckResults::new(OwnerId { def_id: key });

    tcx.arena.alloc(results)
}
#[allow(unused)]
fn analysis<'tcx>(
    _tcx: TyCtxt<'tcx>,
    _key: queries::analysis::LocalKey,
) -> queries::analysis::ProvidedValue<'tcx> {
}
fn default_mir_borrowck<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
) -> queries::mir_borrowck::ProvidedValue<'tcx> {
    let mut providers = Providers::default();
    rustc_borrowck::provide(&mut providers);
    (providers.mir_borrowck)(tcx, def_id)
}
fn mir_borrowck<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
) -> queries::mir_borrowck::ProvidedValue<'tcx> {
    //let key = tcx.def_path_str(def_id.to_def_id());

    log::info!("start borrowck of {def_id:?}");
    //STAT_LOG.with(|f| f.borrow()(&format!("{key},start")));

    if tcx.hir().body_const_context(def_id.to_def_id()).is_some() {
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

    let mut compiling_mir = Vec::with_capacity(1024);
    let mut compiling_mir_str = String::new();
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
            compiling_mir_str = unsafe { String::from_utf8_unchecked(compiling_mir) }
                .trim()
                .to_owned();
            if MIR_CACHE.with(|p| { if let Ok(cache) = unsafe { &**p }.read() {
                log::info!("unsafe deref");
                if cache.contains(&compiling_mir_str) {
                    log::info!("{def_id:?} cache hit");
                    //STAT_LOG.with(|f| f.borrow()(&format!("{key},cache_hit")));
                    return true
                }
            } false
            }) {
                return tcx.arena.alloc(empty_result);
            }
        }
    }

    log::info!("{def_id:?} no cache; start mir_borrowck");

    let result = default_mir_borrowck(tcx, def_id);
    let can_cache = //result.concrete_opaque_types.is_empty()
        //&& result.closure_requirements.is_none()
        //&& result.used_mut_upvars.is_empty()
        result.tainted_by_errors.is_none()
        && 0 < compiling_mir_str.len();
    if can_cache {
        if let Ok(mut cache) = MIR_CACHE.with(|p| unsafe { &**p }.write()) {
            cache.insert(compiling_mir_str.to_owned());
            //STAT_LOG.with(|f| f.borrow()(&format!("{key},cached")));
        }
    } else {
        log::info!("{def_id:?} cannot be cached due to its mir_borrowck result")
    }
    //STAT_LOG.with(|f| f.borrow()(&format!("{def_id:?},no_cache")));
    result
}
#[allow(unused)]
fn check_liveness<'tcx>(_tcx: TyCtxt<'tcx>, _def_id: LocalDefId) {}

pub struct FustcCallback;
impl Callbacks for FustcCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.using_internal_features = &ATOMIC_TRUE;
        config.override_queries = Some(override_queries);
    }
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let cache_path = MIR_CACHE_PATH.with(|cache_path| cache_path.clone());
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            //.truncate(true)
            .open(cache_path)
        //.await
        {
            if let Some(cache) = {
                MIR_CACHE
                    .with(|p| unsafe { &**p }.read())
                    .map(|v| v.clone())
                    .ok()
            } {
                let cache_str = cache
                    .into_iter()
                    .map(|v| v.trim().to_string())
                    .collect::<Vec<String>>()
                    .join("\n\n\n");
                file.write_all(cache_str.as_bytes()).unwrap();
                file.write_all(b"\n\n\n").unwrap();
            }
        } else {
            log::warn!("write cache file failed");
        }
        Compilation::Continue
    }
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

    let mut callback = RustcCallback;
    if compiler == Compiler::Normal {
        return rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut callback));
    }
    for arg in args {
        if arg == "-vV" || arg.starts_with("--print") {
            return rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut callback));
        }
    }
    let mut callback = FustcCallback;
    rustc_driver::catch_with_exit_code(|| run_compiler(&args, &mut callback))
}

fn main() {
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
        _ => run_fustc(Compiler::Normal),
    };

    //HANDLE.block_on(async { while let Some(_) = TASKS.lock().unwrap().join_next().await {} });

    std::process::exit(code);
}
