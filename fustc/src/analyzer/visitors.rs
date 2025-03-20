use rustc_hir::{
    ExprKind,
    hir_id::HirId,
    intravisit::{FnKind, HirTyCtxt, Visitor, walk_body, walk_fn_decl},
};
use rustc_middle::{
    hir::{self, nested_filter},
    ty::TyCtxt,
};
use std::collections::HashSet;

pub fn hir_id_string(tcx: &dyn HirTyCtxt<'_>, id: HirId) -> String {
    rustc_hir_pretty::id_to_string(tcx, id)
}

pub struct HirFnVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    paths: HashSet<String>,
}

impl<'tcx> HirFnVisitor<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            paths: HashSet::new(),
        }
    }
}
impl HirFnVisitor<'_> {
    pub fn get_snippet(&self, id: rustc_hir::BodyId) -> String {
        let span = self.tcx.hir().span_with_body(id.hir_id);
        let source_map = self.tcx.sess.source_map();
        let snippet = source_map.span_to_snippet(span).unwrap();
        snippet
    }
}

impl<'hir> Visitor<'hir> for HirFnVisitor<'hir> {
    type NestedFilter = nested_filter::OnlyBodies;
    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_nested_body(&mut self, id: rustc_hir::BodyId) -> Self::Result {
        let snippet = hir_id_string(&self.tcx, self.tcx.hir_body_owner(id));
        //println!("{snippet}");
        let body_node = self.tcx.hir_node(id.hir_id);
        //let fn_kind = body_node.fn_kind().unwrap();
        let body = self.tcx.hir_body(id);
        let mut walker = HirFnWalker::new(self.tcx);
        if let Some(fn_decl) = body_node.fn_decl() {
            walk_fn_decl(&mut walker, fn_decl);
        }
        walk_body(&mut walker, body);

        println!("{:?}", walker.get_paths());
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
    paths: HashSet<String>,
    locals: HashSet<String>,
}

impl<'tcx> HirFnWalker<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            paths: HashSet::new(),
            locals: HashSet::new(),
        }
    }
    pub fn get_paths(self) -> HashSet<String> {
        self.paths
            .difference(&self.locals)
            .map(|v| v.clone())
            .collect()
    }
    fn walk_pat(&mut self, pat: &'tcx rustc_hir::Pat<'tcx>) -> HashSet<String> {
        let mut locals = HashSet::new();
        pat.walk_always(|pat| {
            if let Some(ident) = pat.simple_ident() {
                locals.insert(ident.to_string());
            }
        });
        locals
    }
}

impl<'hir> Visitor<'hir> for HirFnWalker<'hir> {
    type NestedFilter = nested_filter::All;
    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_qpath(
        &mut self,
        qpath: &'hir rustc_hir::QPath<'hir>,
        id: HirId,
        _span: rustc_span::Span,
    ) -> Self::Result {
        self.paths.insert(hir_id_string(&self.tcx, id));
        //println!("{qpath:?}");
    }
    fn visit_path(&mut self, path: &rustc_hir::Path<'hir>, _id: HirId) -> Self::Result {
        //self.paths.insert(super::path::format_path(path));
    }

    fn visit_ty(&mut self, t: &'hir rustc_hir::Ty<'hir, rustc_hir::AmbigArg>) -> Self::Result {}
    fn visit_expr(&mut self, ex: &'hir rustc_hir::Expr<'hir>) -> Self::Result {
        match ex.kind {
            ExprKind::Call(callee, _args) => {
                callee;
            }
            _ => {}
        }
    }

    fn visit_param(&mut self, param: &'hir rustc_hir::Param<'hir>) -> Self::Result {
        let locals = self.walk_pat(param.pat);
        self.locals.extend(locals);
    }
    fn visit_local(&mut self, l: &'hir rustc_hir::LetStmt<'hir>) -> Self::Result {
        let locals = self.walk_pat(l.pat);
        self.locals.extend(locals);
    }
}
