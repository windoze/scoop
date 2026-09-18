use std::fmt;

use scoop_identity::PersistentExportBindingId;
use scoop_wire::WireError;

use crate::{
    DependencyBindingWitnessSetBuildError, ExternalHirReferenceBuildError,
    ExternalHirReferenceRoleSetBuildError, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSetBuildError, ExternalHirTargetV1,
};

#[derive(Debug)]
pub enum ExternalHirReferenceProductionError<E> {
    MissingBindingKey {
        binding_index: usize,
        binding: PersistentExportBindingId,
    },
    TargetOrigin {
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
        source: E,
    },
    CurrentReexportTarget {
        binding_index: usize,
        target: ExternalHirTargetV1,
    },
    CurrentWitnessTarget {
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
    },
    UnexpectedWitnessUse {
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
    },
    MissingWitnessUse {
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
    },
    Resource(WireError),
    Roles(ExternalHirReferenceRoleSetBuildError),
    Witnesses(DependencyBindingWitnessSetBuildError),
    Record(ExternalHirReferenceBuildError),
    Table(ExternalHirReferenceSetBuildError),
}

impl<E: fmt::Display> fmt::Display for ExternalHirReferenceProductionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBindingKey {
                binding_index,
                binding,
            } => write!(
                formatter,
                "public re-export binding {binding} at index {binding_index} has no canonical key"
            ),
            Self::TargetOrigin {
                target,
                role,
                source,
            } => write!(
                formatter,
                "cannot resolve origin of external HIR target {target:?} for role {role:?}: {source}"
            ),
            Self::CurrentReexportTarget {
                binding_index,
                target,
            } => write!(
                formatter,
                "public re-export binding at index {binding_index} targets current-Cone entity {target:?}"
            ),
            Self::CurrentWitnessTarget { target, role } => write!(
                formatter,
                "binding witness for role {role:?} targets current-Cone entity {target:?}"
            ),
            Self::UnexpectedWitnessUse { target, role } => write!(
                formatter,
                "binding witness declares role {role:?} for target {target:?} without a matching interface use"
            ),
            Self::MissingWitnessUse { target, role } => write!(
                formatter,
                "foreign HIR target {target:?} with role {role:?} has no selected binding witness"
            ),
            Self::Resource(source) => write!(
                formatter,
                "external HIR reference production exceeded its semantic resource budget: {source}"
            ),
            Self::Roles(source) => source.fmt(formatter),
            Self::Witnesses(source) => source.fmt(formatter),
            Self::Record(source) => source.fmt(formatter),
            Self::Table(source) => source.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExternalHirReferenceProductionError<E> {}
