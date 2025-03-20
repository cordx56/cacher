use std::fmt::write;

use rustc_hir::Path;

pub struct DisplayPath<'hir, 'p>(&'p Path<'hir>);
impl std::fmt::Display for DisplayPath<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for seg in self.0.segments {
            write!(f, "::")?;
            write!(f, "{}", seg.ident)?;
        }
        Ok(())
    }
}

pub fn format_path(path: &Path<'_>) -> String {
    format!("{}", DisplayPath(path))
}
