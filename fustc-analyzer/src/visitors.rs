use rustc_hir::{
    hir_id::HirId,
    intravisit::{FnKind, Map, Visitor, walk_body},
};
use rustc_middle::{
    hir::{self, nested_filter},
    ty::TyCtxt,
};

pub struct HirFnVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> HirFnVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self { tcx }
    }
}
impl HirFnVisitor<'_> {
    pub fn get_snippet(&self, id: rustc_hir::BodyId) -> String {
        let span = self.tcx.hir().span_with_body(id.hir_id);
        let source_map = self.tcx.sess.source_map();
        let snippet = source_map.span_to_snippet(span).unwrap();
        snippet
    }
    pub fn get_hir_string(&self, id: HirId) -> String {
        rustc_hir_pretty::id_to_string(&self.tcx.hir(), id)
    }
}

impl<'hir> Visitor<'hir> for HirFnVisitor<'hir> {
    type Map = hir::map::Map<'hir>;
    type NestedFilter = nested_filter::OnlyBodies;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_nested_body(&mut self, id: rustc_hir::BodyId) -> Self::Result {
        let map = self.nested_visit_map();
        let snippet = self.get_hir_string(map.body_owner(id));
        //println!("{snippet}");
        let body_node = map.hir_node(id.hir_id);
        //let fn_kind = body_node.fn_kind().unwrap();
        //let fn_decl = body_node.fn_decl().unwrap();
        let body = self.nested_visit_map().body(id);
        let mut walker = HirFnWalker::new(self.tcx);
        walk_body(&mut walker, body);
        //println!("{id:?}");
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

pub struct HirFnWalker<'tcx> {
    tcx: TyCtxt<'tcx>,
}
impl<'tcx> HirFnWalker<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self { tcx }
    }
}
impl<'hir> Visitor<'hir> for HirFnWalker<'hir> {
    type Map = hir::map::Map<'hir>;
    type NestedFilter = nested_filter::All;
    fn nested_visit_map(&mut self) -> Self::Map {
        self.tcx.hir()
    }

    fn visit_qpath(
        &mut self,
        qpath: &'hir rustc_hir::QPath<'hir>,
        id: HirId,
        _span: rustc_span::Span,
    ) -> Self::Result {
        println!("{qpath:?}");
    }
}
