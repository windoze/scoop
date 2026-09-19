use scoop_identity::SignatureTypeKey;
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::{CanonicalBinderUseListV1, DefaultTemplateTypeSubstitutionError};
use crate::DefaultTemplateProviderShapeV1;

mod build;
#[cfg(test)]
mod tests;
mod walk;

enum Task<'a> {
    Visit {
        value: &'a SignatureTypeKey,
        substitute: bool,
        depth: u64,
    },
    Finish(&'a SignatureTypeKey),
}
struct Substitution<'a, 'm> {
    mapping: &'a CanonicalBinderUseListV1,
    provider: DefaultTemplateProviderShapeV1,
    tasks: Vec<Task<'a>>,
    values: Vec<SignatureTypeKey>,
    meter: &'m mut BudgetMeter,
    path: &'m WirePath,
}
impl CanonicalBinderUseListV1 {
    /// Substitutes provider binders once, charging every expanded output node.
    /// Mapping arguments already belong to the publishing owner's scope.
    pub fn substitute_provider_type_metered(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        signature: &SignatureTypeKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<SignatureTypeKey, MeteredDefaultTemplateTypeSubstitutionError> {
        if self.len_u32() != provider.binder_arity() {
            return Err(DefaultTemplateTypeSubstitutionError::MappingArity {
                expected: provider.binder_arity(),
                actual: self.len_u32(),
            }
            .into());
        }
        let mut walk = Substitution {
            mapping: self,
            provider,
            tasks: Vec::new(),
            values: Vec::new(),
            meter,
            path,
        };
        walk.meter
            .try_reserve_collection_slots(&mut walk.tasks, 1, path)?;
        walk.meter
            .try_reserve_collection_slots(&mut walk.values, 1, path)?;
        walk.tasks.push(Task::Visit {
            value: signature,
            substitute: true,
            depth: 1,
        });
        while let Some(task) = walk.tasks.pop() {
            match task {
                Task::Visit {
                    value,
                    substitute,
                    depth,
                } => walk.visit(value, substitute, depth)?,
                Task::Finish(value) => walk.finish(value)?,
            }
        }
        if walk.values.len() != 1 {
            return Err(invalid_length(1, walk.values.len(), path).into());
        }
        walk.values
            .pop()
            .ok_or_else(|| invalid_length(1, 0, path).into())
    }
}
fn invalid_length(expected: usize, actual: usize, path: &WirePath) -> WireError {
    WireError::new(
        WireErrorKind::InvalidLength {
            expected: expected as u64,
            actual: actual as u64,
        },
        path.clone(),
        None,
    )
}
fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}

#[derive(Debug, Eq, PartialEq)]
pub enum MeteredDefaultTemplateTypeSubstitutionError {
    Resource(WireError),
    Substitution(DefaultTemplateTypeSubstitutionError),
}
impl From<WireError> for MeteredDefaultTemplateTypeSubstitutionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<DefaultTemplateTypeSubstitutionError> for MeteredDefaultTemplateTypeSubstitutionError {
    fn from(error: DefaultTemplateTypeSubstitutionError) -> Self {
        Self::Substitution(error)
    }
}
impl std::fmt::Display for MeteredDefaultTemplateTypeSubstitutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Substitution(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for MeteredDefaultTemplateTypeSubstitutionError {}
