use std::cmp::Ordering;

use scoop_identity::{BindingTarget, ConeIdentity, PersistentExportBindingId};

use super::{ImportedProviderCertificate, ImportedTarget, WorldConeId};
use crate::{
    DependencyBindingWitnessV1, ExportBindingSourceV1, ImportedHirId, ReexportRouteBuildError,
    ReexportRouteHopV1, ReexportRouteV1,
};

/// Opaque proof that one imported target is reachable through a validated
/// public binding of a direct dependency.
///
/// Construction stays inside the semantic world so a typed entity obtained
/// only from a support provider cannot be promoted into ordinary lookup.
#[derive(Clone, Debug)]
pub struct PublicDependencyLookupWitness {
    terminal_declaration: ImportedTarget,
    dependency: DependencyBindingWitnessV1,
}

impl PublicDependencyLookupWitness {
    pub const fn terminal_declaration(&self) -> ImportedTarget {
        self.terminal_declaration
    }

    pub const fn dependency(&self) -> &DependencyBindingWitnessV1 {
        &self.dependency
    }

    pub const fn route(&self) -> &ReexportRouteV1 {
        self.dependency.route()
    }
}

/// One canonical direct-dependency source for an imported target.
///
/// The session-local provider handle is deliberately excluded from canonical
/// ordering. The retained certificate supplies stable coordinate and
/// fingerprint information for diagnostics and later artifact reopening.
#[derive(Clone, Debug)]
pub struct DirectDependencyImportSource {
    immediate_provider: WorldConeId,
    certificate: ImportedProviderCertificate,
    exported_binding: ImportedHirId<PersistentExportBindingId>,
    witness: PublicDependencyLookupWitness,
}

impl DirectDependencyImportSource {
    pub const fn immediate_provider(&self) -> WorldConeId {
        self.immediate_provider
    }

    pub const fn provider_identity(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn exported_binding(&self) -> ImportedHirId<PersistentExportBindingId> {
        self.exported_binding
    }

    pub const fn witness(&self) -> &PublicDependencyLookupWitness {
        &self.witness
    }

    pub(super) fn from_validated_binding(
        immediate_provider: WorldConeId,
        certificate: &ImportedProviderCertificate,
        exported_binding: ImportedHirId<PersistentExportBindingId>,
        target: ImportedTarget,
        source: &ExportBindingSourceV1,
    ) -> Result<Vec<Self>, ReexportRouteBuildError> {
        let provider_identity = certificate.identity();
        let routes = match source {
            ExportBindingSourceV1::DeclaredCurrent { .. } => vec![ReexportRouteV1::try_new(
                provider_identity,
                vec![ReexportRouteHopV1::new(
                    provider_identity,
                    exported_binding.persistent(),
                )],
            )?],
            ExportBindingSourceV1::Reexport { routes } => routes
                .routes()
                .iter()
                .map(|route| {
                    let mut hops = Vec::with_capacity(route.hops().len() + 1);
                    hops.push(ReexportRouteHopV1::new(
                        provider_identity,
                        exported_binding.persistent(),
                    ));
                    hops.extend_from_slice(route.hops());
                    ReexportRouteV1::try_new(provider_identity, hops)
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(routes
            .into_iter()
            .map(|route| Self {
                immediate_provider,
                certificate: certificate.clone(),
                exported_binding,
                witness: PublicDependencyLookupWitness {
                    terminal_declaration: target,
                    dependency: DependencyBindingWitnessV1::new(route),
                },
            })
            .collect())
    }

    pub(super) fn canonical_cmp(&self, other: &Self) -> Ordering {
        coordinate_key(&self.certificate)
            .cmp(&coordinate_key(&other.certificate))
            .then_with(|| self.provider_identity().cmp(&other.provider_identity()))
            .then_with(|| {
                self.exported_binding
                    .persistent()
                    .cmp(&other.exported_binding.persistent())
            })
            .then_with(|| self.witness.route().cmp(other.witness.route()))
    }

    pub(super) fn canonical_eq(&self, other: &Self) -> bool {
        self.canonical_cmp(other).is_eq()
    }
}

fn coordinate_key(certificate: &ImportedProviderCertificate) -> (&str, &str, &str) {
    let coordinate = certificate.coordinate();
    (coordinate.group(), coordinate.name(), coordinate.version())
}

/// One typed target together with every direct source that authorizes it.
/// The private source vector is always non-empty and canonical.
#[derive(Clone, Debug)]
pub struct DirectImportedTargetBinding {
    binding_target: BindingTarget,
    target: ImportedTarget,
    sources: Vec<DirectDependencyImportSource>,
}

impl DirectImportedTargetBinding {
    pub const fn binding_target(&self) -> BindingTarget {
        self.binding_target
    }

    pub const fn target(&self) -> ImportedTarget {
        self.target
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &DirectDependencyImportSource> + '_ {
        self.sources.iter()
    }

    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub(super) fn new(
        binding_target: BindingTarget,
        target: ImportedTarget,
        mut sources: Vec<DirectDependencyImportSource>,
    ) -> Self {
        assert!(
            !sources.is_empty(),
            "an imported target binding has at least one direct source"
        );
        sources.sort_unstable_by(DirectDependencyImportSource::canonical_cmp);
        sources.dedup_by(|left, right| left.canonical_eq(right));
        Self {
            binding_target,
            target,
            sources,
        }
    }
}
