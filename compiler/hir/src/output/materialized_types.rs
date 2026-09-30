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
        self.materialized_types()?.into_types()
    }

    fn materialized_types(&self) -> Result<Collector<'_>, MaterializedTypeClosureError> {
        let mut collector = Collector::new(self.module());
        collector.roots(self)?;
        self.module()
            .visit_executable_expressions(|occurrence| collector.expression(occurrence.expression))
            .map_err(|error| match error {
                ExecutableExpressionVisitError::Structure(error) => {
                    MaterializedTypeClosureError::Executable(error)
                }
                ExecutableExpressionVisitError::Visitor(error) => error,
            })?;
        collector.close()
    }

    /// Shared representations, member ABIs and dependency receiver relations
    /// need actual declarations; unrelated private declarations remain local.
    pub(crate) fn shared_declaration_type_closure(
        &self,
    ) -> Result<Vec<TypeId>, MaterializedTypeClosureError> {
        let materialized = self.materialized_types()?;
        let mut collector = Collector::new(self.module());
        for ty in materialized.seen {
            if materialized.shared_types.contains(&ty)
                || self
                    .module()
                    .exact_type_identities
                    .nominal_specialization(ty)
                    .is_some()
            {
                collector.add(ty)?;
            }
        }
        for (_, function) in self.module().functions.iter() {
            if matches!(
                function.materialization.context(),
                CallableMaterializationContext::Application(_)
            ) && function.receiver.method().is_some()
                && matches!(
                    function.materialization.template(),
                    CallableTemplateOwner::Function(_)
                        | CallableTemplateOwner::GenericFunction(_)
                        | CallableTemplateOwner::Accessor(_)
                )
            {
                collector.signature(function)?;
            }
        }
        collector.close()?.into_types()
    }
}

struct Collector<'a> {
    module: &'a Module,
    seen: BTreeSet<TypeId>,
    pending: Vec<TypeId>,
    shared_types: BTreeSet<TypeId>,
}

impl<'a> Collector<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            seen: BTreeSet::new(),
            pending: Vec::new(),
            shared_types: BTreeSet::new(),
        }
    }

    fn close(mut self) -> Result<Self, MaterializedTypeClosureError> {
        while let Some(ty) = self.pending.pop() {
            self.children(ty)?;
        }
        Ok(self)
    }

    fn into_types(self) -> Result<Vec<TypeId>, MaterializedTypeClosureError> {
        let mut result = Vec::new();
        scoop_wire::allocation::try_reserve(&mut result, self.seen.len(), &WirePath::root())?;
        result.extend(self.seen);
        Ok(result)
    }

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
