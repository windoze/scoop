use std::fmt;

use scoop_identity::{BindingTarget, ConeIdentity, PersistentExportBindingId};

use super::{ImportedBindingConflictKey, ImportedTarget};
use crate::{
    DependencyBindingWitnessV1, ExportBindingSourceV1, ReexportRouteBuildError, ReexportRouteHopV1,
    ReexportRouteV1,
};

pub(super) fn import_binding_routes(
    provider: ConeIdentity,
    binding: PersistentExportBindingId,
    source: &ExportBindingSourceV1,
) -> Result<Vec<DependencyBindingWitnessV1>, ReexportRouteBuildError> {
    let routes = match source {
        ExportBindingSourceV1::DeclaredCurrent { .. } => vec![ReexportRouteV1::try_new(
            provider,
            vec![ReexportRouteHopV1::new(provider, binding)],
        )?],
        ExportBindingSourceV1::Reexport { routes } => routes
            .routes()
            .iter()
            .map(|route| {
                let mut hops = Vec::with_capacity(route.hops().len() + 1);
                hops.push(ReexportRouteHopV1::new(provider, binding));
                hops.extend_from_slice(route.hops());
                ReexportRouteV1::try_new(provider, hops)
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(routes
        .into_iter()
        .map(DependencyBindingWitnessV1::new)
        .collect())
}

/// One typed target with references to its selected public bindings.
/// Static-owner lookup may select a binding from a support provider.
/// The private source vector is always non-empty and canonical.
#[derive(Clone, Debug)]
pub struct DirectImportedTargetBinding {
    binding_target: BindingTarget,
    target: ImportedTarget,
    conflict: ImportedBindingConflictKey,
    sources: Vec<DependencyBindingWitnessV1>,
}

impl DirectImportedTargetBinding {
    pub const fn binding_target(&self) -> BindingTarget {
        self.binding_target
    }

    pub const fn target(&self) -> ImportedTarget {
        self.target
    }

    pub const fn conflict_key(&self) -> &ImportedBindingConflictKey {
        &self.conflict
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &DependencyBindingWitnessV1> + '_ {
        self.sources.iter()
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub fn try_merge(&mut self, other: Self) -> Result<(), DirectImportedTargetMergeError> {
        if self.binding_target != other.binding_target {
            return Err(DirectImportedTargetMergeError::BindingTargetMismatch);
        }
        if self.target != other.target {
            return Err(DirectImportedTargetMergeError::ImportedTargetMismatch);
        }
        if self.conflict != other.conflict {
            return Err(DirectImportedTargetMergeError::ConflictKeyMismatch);
        }
        self.sources.extend(other.sources);
        self.sources
            .sort_unstable_by(|left, right| left.route().cmp(right.route()));
        self.sources.dedup();
        Ok(())
    }

    pub(super) fn new(
        binding_target: BindingTarget,
        target: ImportedTarget,
        conflict: ImportedBindingConflictKey,
        mut sources: Vec<DependencyBindingWitnessV1>,
    ) -> Self {
        assert!(
            !sources.is_empty(),
            "an imported target binding has at least one public binding source"
        );
        sources.sort_unstable_by(|left, right| left.route().cmp(right.route()));
        sources.dedup();
        Self {
            binding_target,
            target,
            conflict,
            sources,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectImportedTargetMergeError {
    BindingTargetMismatch,
    ImportedTargetMismatch,
    ConflictKeyMismatch,
}

impl fmt::Display for DirectImportedTargetMergeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindingTargetMismatch => {
                formatter.write_str("direct imported bindings name different binding targets")
            }
            Self::ImportedTargetMismatch => {
                formatter.write_str("direct imported bindings refer to different declarations")
            }
            Self::ConflictKeyMismatch => formatter
                .write_str("direct imported bindings disagree on the target's public conflict key"),
        }
    }
}

impl std::error::Error for DirectImportedTargetMergeError {}
