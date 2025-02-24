#![feature(rustc_private)]

mod ty;
mod visitors;

extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_middle;
extern crate rustc_span;

pub use visitors::{HirFnVisitor, HirFnWalker};
