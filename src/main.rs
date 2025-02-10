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
    mir::{write_mir_pretty, BorrowCheckResult},
    query::queries,
    ty::{TyCtxt, TypeckResults},
    util::Providers,
};
use rustc_session::{config, EarlyDiagCtxt};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{atomic::AtomicBool, Arc, LazyLock, Mutex};

pub struct RustcCallback;
impl Callbacks for RustcCallback {}

fn override_queries(_session: &rustc_session::Session, local: &mut Providers) {
    //local.analysis = analysis;
    local.mir_borrowck = mir_borrowck;
    local.check_liveness = check_liveness;
    //local.typeck = typeck;
}
fn typeck<'tcx>(
    tcx: TyCtxt<'tcx>,
    key: queries::typeck::LocalKey<'tcx>,
) -> queries::typeck::ProvidedValue<'tcx> {
    let results = TypeckResults::new(OwnerId { def_id: key });

    tcx.arena.alloc(results)
}
fn analysis<'tcx>(
    tcx: TyCtxt<'tcx>,
    key: queries::analysis::LocalKey,
) -> queries::analysis::ProvidedValue<'tcx> {
    Ok(())
}
fn mir_borrowck<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: LocalDefId,
) -> queries::mir_borrowck::ProvidedValue<'tcx> {
    log::info!("start borrowck of {def_id:?}");

    let mut compiling_mir = Vec::with_capacity(1024);
    write_mir_pretty(tcx, Some(def_id.into()), &mut compiling_mir).unwrap();

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .read(true)
        .open(format!(".mir/{def_id:?}"))
    {
        let mut cached_mir = Vec::with_capacity(1024);
        file.read_to_end(&mut cached_mir).unwrap();
        if cached_mir == compiling_mir {
            log::info!("{def_id:?} cache hit");

            let result = BorrowCheckResult {
                concrete_opaque_types: indexmap::IndexMap::default(),
                closure_requirements: None,
                used_mut_upvars: smallvec::SmallVec::new(),
                tainted_by_errors: None,
            };
            return tcx.arena.alloc(result);
        }
    }

    log::info!("{def_id:?} no cache");

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(format!(".mir/{def_id:?}"))
        .unwrap();
    file.write_all(&compiling_mir).unwrap();

    let mut providers = Providers::default();
    rustc_borrowck::provide(&mut providers);
    (providers.mir_borrowck)(tcx, def_id)
}
fn check_liveness<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) {}

pub struct AnalyzerCallback;
impl Callbacks for AnalyzerCallback {
    fn config(&mut self, config: &mut interface::Config) {
        config.opts.unstable_opts.mir_opt_level = Some(0);
        config.opts.unstable_opts.polonius = config::Polonius::Next;
        config.opts.incremental = None;
        config.override_queries = Some(override_queries);
        config.make_codegen_backend = None;
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
    let fast_result = std::panic::catch_unwind(|| run_compiler(Compiler::Fast));
    match fast_result {
        Ok(0) => {}
        _ => std::process::exit(run_compiler(Compiler::Normal)),
    }
}
