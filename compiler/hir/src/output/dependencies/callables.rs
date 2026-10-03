//! Machine callable roots come from executable expressions and dispatch tables.

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
    /// Reuses the actual machine roots collected when this immutable output was
    /// built. Source-only default references do not become executable roots.
    pub fn executable_dependency_callables(
        &self,
    ) -> Result<Vec<ExecutableDependencyCallableUse<'_>>, DependencyCallOccurrenceError> {
        let mut uses = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut uses,
            self.executable_callables.len(),
            &WirePath::root(),
        )?;
        for &callee in &self.executable_callables {
            let reference =
                self.output.local.module().imported_dependency_callables[callee].reference();
            let callable = self
                .imported_dependencies
                .resolve_callable(reference)
                .expect("output construction resolved every executable dependency callable");
            uses.push(ExecutableDependencyCallableUse { callee, callable });
        }
        Ok(uses)
    }
}
