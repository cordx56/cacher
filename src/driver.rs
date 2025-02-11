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

use rustc_driver::{Callbacks, RunCompiler};
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
use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, LazyLock, RwLock};

thread_local! {
    static MIR_JSON_PATH: PathBuf = env::var("FUSTC_CWD")
        .map(|v| PathBuf::from(v))
        .unwrap_or(env::current_dir().unwrap())
        .join(".mir.json");
}
static MIR_CACHE: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

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

pub struct RustcCallback;
impl Callbacks for RustcCallback {}

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    //local.analysis = analysis;
    local.mir_borrowck = mir_borrowck;
    local.check_liveness = check_liveness;
    //local.typeck = typeck;
}
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
    Ok(())
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
    log::info!("start borrowck of {def_id:?}");
    STAT_LOG.with(|f| f.borrow()(&format!("{def_id:?},start")));

    if tcx.hir().body_const_context(def_id.to_def_id()).is_some() {
        let result = default_mir_borrowck(tcx, def_id);
        STAT_LOG.with(|f| f.borrow()(&format!("{def_id:?},no_cache")));
        return result;
    }

    let empty_result = BorrowCheckResult {
        concrete_opaque_types: indexmap::IndexMap::default(),
        closure_requirements: None,
        used_mut_upvars: smallvec::SmallVec::new(),
        tainted_by_errors: None,
    };

    let mut compiling_mir = Vec::with_capacity(1024);
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
            if let Ok(cache) = MIR_CACHE.read() {
                if let Some(cached_mir) = cache.get(&format!("{def_id:?}")) {
                    if cached_mir.as_bytes() == &compiling_mir {
                        log::info!("{def_id:?} cache hit");
                        STAT_LOG.with(|f| f.borrow()(&format!("{def_id:?},cache_hit")));
                        return tcx.arena.alloc(empty_result);
                    }
                }
            }
        }
    }

    log::info!("{def_id:?} no cache; start mir_borrowck");

    let result = default_mir_borrowck(tcx, def_id);
    let can_cache = /*result.concrete_opaque_types.is_empty()
        && result.closure_requirements.is_none()
        && result.used_mut_upvars.is_empty()
        &&*/ result.tainted_by_errors.is_none()
        && 0 < compiling_mir.len();
    if can_cache {
        if let Ok(mut cache) = MIR_CACHE.write() {
            cache.insert(
                format!("{def_id:?}"),
                String::from_utf8_lossy(&compiling_mir).to_string(),
            );
            STAT_LOG.with(|f| f.borrow()(&format!("{def_id:?},cached")));
        }
    } else {
        log::info!("{def_id:?} cannot be cached due to its mir_borrowck result")
    }
    STAT_LOG.with(|f| f.borrow()(&format!("{def_id:?},no_cache")));
    result
}
fn check_liveness<'tcx>(_tcx: TyCtxt<'tcx>, _def_id: LocalDefId) {}

pub struct AnalyzerCallback;
impl Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.override_queries = Some(override_queries);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compiler {
    Normal,
    Fast,
}

pub fn run_compiler(compiler: Compiler) -> i32 {
    let ctxt = EarlyDiagCtxt::new(config::ErrorOutputType::default());
    let args = rustc_driver::args::raw_args(&ctxt).unwrap();
    let args = &args[1..];

    let mut callback = RustcCallback;
    let runner = RunCompiler::new(&args, &mut callback);
    if compiler == Compiler::Normal {
        return rustc_driver::catch_with_exit_code(|| runner.run());
    }
    for arg in args {
        if arg == "-vV" || arg.starts_with("--print") {
            return rustc_driver::catch_with_exit_code(|| runner.run());
        }
    }
    let mut callback = AnalyzerCallback;
    let mut runner = RunCompiler::new(&args, &mut callback);
    runner.set_make_codegen_backend(None);
    rustc_driver::catch_with_exit_code(|| {
        runner
            .set_using_internal_features(Arc::new(AtomicBool::new(true)))
            .run()
    })
}

fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(env::var("FUSTC_LOG").map_or(log::LevelFilter::Error, |v| {
            v.parse().unwrap_or(log::LevelFilter::Error)
        }))
        .with_colors(true)
        .init()
        .unwrap();

    MIR_JSON_PATH.with(|json_path| {
        if let Ok(mut file) = File::open(json_path) {
            let mut json = Vec::with_capacity(1024);
            file.read_to_end(&mut json).unwrap();
            if let Ok(mut cache) = MIR_CACHE.write() {
                if let Ok(data) = serde_json::from_slice(&json) {
                    *cache = data;
                }
            }
        }
    });

    let fast_result = std::panic::catch_unwind(|| run_compiler(Compiler::Fast));
    let code = match fast_result {
        Ok(0) => 0,
        _ => run_compiler(Compiler::Normal),
    };

    MIR_JSON_PATH.with(|json_path| {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(json_path)
        {
            if let Ok(cache) = MIR_CACHE.read() {
                let json = serde_json::to_string(&*cache).unwrap();
                file.write_all(json.as_bytes()).unwrap();
            }
        }
    });

    std::process::exit(code);
}
