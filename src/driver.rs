#![feature(rustc_private)]

pub extern crate indexmap;
pub extern crate polonius_engine;
pub extern crate rustc_borrowck;
pub extern crate rustc_driver;
pub extern crate rustc_errors;
pub extern crate rustc_hash;
pub extern crate rustc_hir;
pub extern crate rustc_index;
pub extern crate rustc_interface;
pub extern crate rustc_middle;
pub extern crate rustc_session;
pub extern crate rustc_span;
pub extern crate smallvec;

pub extern crate rustc_hir_typeck;

pub use fustc_analyzer;

use rustc_driver::{run_compiler, Callbacks, Compilation};
use rustc_hir::{def_id::LocalDefId, hir_id::OwnerId};
use rustc_interface::interface;
use rustc_middle::{
    mir::{
        pretty::{write_mir_fn, PrettyPrintMirOptions},
        BorrowCheckResult,
    },
    query::queries,
    ty::{TyCtxt, TypeckResults},
    util::Providers,
};
use rustc_session::{config, EarlyDiagCtxt};
use std::cell::RefCell;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, LazyLock, RwLock};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    runtime::{Builder, Handle, Runtime},
    task::JoinSet,
};

use rustc_hir::{hir_id::HirId, intravisit::Visitor};

thread_local! {
    static MIR_JSON_PATH: PathBuf = env::var("FUSTC_CWD")
        .map(|v| PathBuf::from(v))
        .unwrap_or(env::current_dir().unwrap())
        .join(".mir.json");
}
static MIR_CACHE: LazyLock<RwLock<HashSet<String>>> = LazyLock::new(|| RwLock::new(HashSet::new()));
static RUNTIME: LazyLock<RwLock<Runtime>> =
    LazyLock::new(|| RwLock::new(Builder::new_multi_thread().enable_all().build().unwrap()));
static HANDLE: LazyLock<Handle> = LazyLock::new(|| RUNTIME.read().unwrap().handle().clone());
static TASKS: LazyLock<RwLock<JoinSet<()>>> = LazyLock::new(|| RwLock::new(JoinSet::new()));

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

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    //local.analysis = analysis;
    local.mir_borrowck = mir_borrowck;
    //local.check_liveness = check_liveness;
    local.typeck = typeck;
}
#[allow(unused)]
fn typeck<'tcx>(
    tcx: TyCtxt<'tcx>,
    key: queries::typeck::LocalKey<'tcx>,
) -> queries::typeck::ProvidedValue<'tcx> {
    let mut visitor = fustc_analyzer::HirFnVisitor::new(tcx);
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

    //log::info!("start borrowck of {def_id:?}");
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
            compiling_mir_str = String::from_utf8(compiling_mir).unwrap();
            //compiling_hash = format!("{:x}", md5::compute(&compiling_mir));
            if let Ok(cache) = MIR_CACHE.read() {
                if cache.contains(&compiling_mir_str) {
                    //log::info!("{def_id:?} cache hit");
                    //STAT_LOG.with(|f| f.borrow()(&format!("{key},cache_hit")));
                    return tcx.arena.alloc(empty_result);
                }
            }
        }
    }

    //log::info!("{def_id:?} no cache; start mir_borrowck");

    let result = default_mir_borrowck(tcx, def_id);
    let can_cache = result.concrete_opaque_types.is_empty()
        && result.closure_requirements.is_none()
        && result.used_mut_upvars.is_empty()
        && result.tainted_by_errors.is_none()
        && 0 < compiling_mir_str.len();
    if can_cache {
        if let Ok(mut cache) = MIR_CACHE.write() {
            cache.insert(compiling_mir_str);
            //STAT_LOG.with(|f| f.borrow()(&format!("{key},cached")));
        }
    } else {
        //log::info!("{def_id:?} cannot be cached due to its mir_borrowck result")
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
    }
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        HANDLE.block_on(async {
            while let Some(_) = { TASKS.write().unwrap().join_next() }.await {}
        });
        Compilation::Continue
    }
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        _tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        TASKS.write().unwrap().spawn_on(
            async {
                let json_path = MIR_JSON_PATH.with(|json_path| json_path.clone());
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(json_path)
                    .await
                {
                    if let Some(cache) = { MIR_CACHE.read().map(|v| v.clone()).ok() } {
                        let json = serde_json::to_string(&cache).unwrap();
                        file.write_all(json.as_bytes()).await.unwrap();
                    }
                }
            },
            &HANDLE,
        );
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

    HANDLE.block_on(async { while let Some(_) = TASKS.write().unwrap().join_next().await {} });

    std::process::exit(code);
}
