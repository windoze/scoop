//! A transient query over actual LocalConcrete HIR, never a use-site permit.

use std::collections::BTreeSet;

use scoop_wire::{WireError, WirePath};

use crate::{LocalConcreteHirOutput, concrete::*};

mod expressions;
mod roots;
mod shapes;

impl LocalConcreteHirOutput {
    /// Replays type requirements from materialized source roots and their
    /// semantic children. The complete identity arena is only a lookup table;
    /// unrelated source metadata and intrinsic declarations are not roots.
    pub fn materialized_type_closure(&self) -> Result<Vec<TypeId>, MaterializedTypeClosureError> {
        let mut collector = Collector {
            module: self.module(),
            seen: BTreeSet::new(),
            pending: Vec::new(),
        };
        collector.roots(self)?;
        self.module()
            .visit_executable_expressions(|occurrence| collector.expression(occurrence.expression))
            .map_err(|error| match error {
                ExecutableExpressionVisitError::Structure(error) => {
                    MaterializedTypeClosureError::Executable(error)
                }
                ExecutableExpressionVisitError::Visitor(error) => error,
            })?;
        while let Some(ty) = collector.pending.pop() {
            collector.children(ty)?;
        }
        let mut result = Vec::new();

        scoop_wire::allocation::try_reserve(&mut result, collector.seen.len(), &WirePath::root())?;
        result.extend(collector.seen);
        Ok(result)
    }
}

struct Collector<'a> {
    module: &'a Module,
    seen: BTreeSet<TypeId>,
    pending: Vec<TypeId>,
}

impl Collector<'_> {
    fn add(&mut self, ty: TypeId) -> Result<(), MaterializedTypeClosureError> {
        let path = WirePath::root();

        if ty.into_raw().into_u32() as usize >= self.module.types.len() {
            return Err(MaterializedTypeClosureError::MissingType(ty));
        }
        if !self.seen.contains(&ty) {
            scoop_wire::allocation::try_reserve(&mut self.pending, 1, &path)?;
            self.seen.insert(ty);
            self.pending.push(ty);
        }
        Ok(())
    }

    fn types(
        &mut self,
        types: impl IntoIterator<Item = TypeId>,
    ) -> Result<(), MaterializedTypeClosureError> {
        for ty in types {
            self.add(ty)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaterializedTypeClosureError {
    Resource(WireError),
    Executable(ExecutableExpressionStructureError),
    MissingType(TypeId),
}

impl From<WireError> for MaterializedTypeClosureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for MaterializedTypeClosureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid materialized HIR type closure: {self:?}")
    }
}

impl std::error::Error for MaterializedTypeClosureError {}
