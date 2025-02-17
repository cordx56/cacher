#![feature(rustc_private)]

mod ty;

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::intravisit::{FnKind, Map, Visitor};
use rustc_middle::{hir::nested_filter, ty::TyCtxt};

pub struct HirFnVisitor<'tcx>(TyCtxt<'tcx>);

impl<'tcx> HirFnVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self(tcx)
    }
}

impl<'hir> Visitor<'hir> for HirFnVisitor<'hir> {
    type NestedFilter = nested_filter::OnlyBodies;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.0.hir()
    }

    fn visit_nested_body(&mut self, id: rustc_hir::BodyId) -> Self::Result {
        let node = self.nested_visit_map().hir_node(id.hir_id);
        if let Some(sig) = node.fn_sig() {

        }
        let body = self.nested_visit_map().body(id);
        println!("{id:?}");
    }
    /*
    fn visit_fn(
        &mut self,
        fk: rustc_hir::intravisit::FnKind<'hir>,
        fd: &'hir rustc_hir::FnDecl<'hir>,
        b: rustc_hir::BodyId,
        _: rustc_span::Span,
        id: rustc_hir::def_id::LocalDefId,
    ) -> Self::Result {
        println!("{id:?}");
    }
    */
}
