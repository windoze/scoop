//! Machine callable roots are borrowed from actual executable expressions.

use scoop_wire::WirePath;

use super::*;

#[derive(Clone, Copy, Debug)]
pub struct ExecutableDependencyCallableUse<'a> {
    callee: concrete::ImportedDependencyCallableUseId,
    callable: &'a crate::SelectedImportedDependencyCallable,
}

impl<'a> ExecutableDependencyCallableUse<'a> {
    pub const fn callee(self) -> concrete::ImportedDependencyCallableUseId {
        self.callee
    }

    pub const fn callable(self) -> &'a crate::SelectedImportedDependencyCallable {
        self.callable
    }
}

impl DependencyHirOutput {
    /// Validates every occurrence before deduplicating machine targets. The
    /// result carries no representative source route: all actual routes were
    /// checked, including later occurrences of an already encountered callee.
    pub fn executable_dependency_callables(
        &self,
    ) -> Result<Vec<ExecutableDependencyCallableUse<'_>>, DependencyCallOccurrenceError> {
        let path = WirePath::root();
        let mut uses = Vec::new();
        occurrences::visit(&self.output, &self.imported_dependencies, |occurrence| {
            scoop_wire::allocation::try_reserve(&mut uses, 1, &path)?;
            uses.push(ExecutableDependencyCallableUse {
                callee: occurrence.callee(),
                callable: occurrence.callable(),
            });
            Ok(())
        })?;

        uses.sort_unstable_by_key(|use_| use_.callee);
        uses.dedup_by_key(|use_| use_.callee);
        Ok(uses)
    }
}
