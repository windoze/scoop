//! Shared winner commit for dependency callables.
//!
//! Name calls, property accessors and calls embedded in provider defaults all
//! converge here after their own source-level applicability checks. Keeping
//! arena interning in one place guarantees that one semantic selection has
//! one local HIR use id regardless of how it was reached.

use scoop_hir as hir;

use crate::Lowerer;

impl Lowerer {
    pub(crate) fn select_imported_dependency_callable_use(
        &mut self,
        candidate: hir::ImportedDependencyCallableCandidate,
    ) -> Result<
        (
            hir::ImportedDependencyCallableUseId,
            std::sync::Arc<hir::DirectImportedTargetBinding>,
        ),
        hir::ImportedDependencySelectionError,
    > {
        let binding = std::sync::Arc::new(candidate.binding().clone());
        let reference = self
            .dependencies
            .as_mut()
            .expect("ordinary lowering carries a dependency selection plan")
            .select_callable(candidate)?;
        let existing = self
            .imported_dependency_callables
            .iter()
            .find_map(|(id, use_)| (use_.reference() == reference).then_some(id));
        let callee = match existing {
            Some(existing) => existing,
            None => self
                .imported_dependency_callables
                .alloc(hir::ImportedDependencyCallableUse::new(reference)),
        };
        Ok((callee, binding))
    }
}
