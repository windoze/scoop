use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    ConeCoordinate, ConeIdentity, PersistentExportBindingId, SemanticOriginFingerprint,
};

use super::{DirectDependencyImportSource, ImportedEntityIndex, ImportedSemanticWorldBuildError};
use crate::{
    CanonicalTypeAliasExpansionsV1, CrossConeHirInterfaceSectionV1, ImportedHirFoundation,
};

mod views;
pub use views::*;

/// Session certificate retained with every imported provider. It owns the
/// data needed to re-open the same artifact after HIR candidate selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedProviderCertificate {
    coordinate: ConeCoordinate,
    identity: ConeIdentity,
    semantic_fingerprint: SemanticOriginFingerprint,
}

impl ImportedProviderCertificate {
    #[doc(hidden)]
    pub const fn from_validated(
        coordinate: ConeCoordinate,
        identity: ConeIdentity,
        semantic_fingerprint: SemanticOriginFingerprint,
    ) -> Self {
        Self {
            coordinate,
            identity,
            semantic_fingerprint,
        }
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }

    pub const fn semantic_fingerprint(&self) -> SemanticOriginFingerprint {
        self.semantic_fingerprint
    }
}

macro_rules! provider_input {
    ($name:ident) => {
        pub struct $name<'input> {
            certificate: ImportedProviderCertificate,
            foundation: &'input ImportedHirFoundation,
            interface: &'input CrossConeHirInterfaceSectionV1,
            alias_expansions: &'input CanonicalTypeAliasExpansionsV1,
        }

        impl<'input> $name<'input> {
            #[doc(hidden)]
            pub const fn from_validated(
                certificate: ImportedProviderCertificate,
                foundation: &'input ImportedHirFoundation,
                interface: &'input CrossConeHirInterfaceSectionV1,
                alias_expansions: &'input CanonicalTypeAliasExpansionsV1,
            ) -> Self {
                Self {
                    certificate,
                    foundation,
                    interface,
                    alias_expansions,
                }
            }
        }
    };
}

provider_input!(TrustedCoreImportedProviderInput);
provider_input!(DirectImportedProviderInput);
provider_input!(SupportImportedProviderInput);

/// Opaque, process-local provider handle. Its world brand prevents handles
/// from one imported closure from being accepted by another.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorldConeId {
    brand: u64,
    index: u32,
}

impl WorldConeId {
    pub(super) const fn new(brand: u64, index: u32) -> Self {
        Self { brand, index }
    }

    pub(super) const fn brand(self) -> u64 {
        self.brand
    }

    pub(super) const fn index(self) -> usize {
        self.index as usize
    }
}

impl fmt::Debug for WorldConeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorldConeId")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

pub(super) enum ProviderSeed<'input> {
    TrustedCore(TrustedCoreImportedProviderInput<'input>),
    Direct(DirectImportedProviderInput<'input>),
    Support(SupportImportedProviderInput<'input>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderSeedRole {
    TrustedCore,
    Direct,
    Support,
}

impl<'input> ProviderSeed<'input> {
    pub(super) fn canonicalize(
        current: ConeIdentity,
        trusted_core: Option<TrustedCoreImportedProviderInput<'input>>,
        direct: Vec<DirectImportedProviderInput<'input>>,
        support: Vec<SupportImportedProviderInput<'input>>,
    ) -> Result<Vec<Self>, ImportedSemanticWorldBuildError> {
        let mut seeds =
            Vec::with_capacity(usize::from(trusted_core.is_some()) + direct.len() + support.len());
        if let Some(core) = trusted_core {
            seeds.push(Self::TrustedCore(core));
        }
        seeds.extend(direct.into_iter().map(Self::Direct));
        seeds.extend(support.into_iter().map(Self::Support));
        for seed in &seeds {
            seed.validate(current)?;
        }
        seeds.sort_unstable_by_key(|seed| seed.certificate().identity());
        if let Some(pair) = seeds
            .windows(2)
            .find(|pair| pair[0].certificate().identity() == pair[1].certificate().identity())
        {
            return Err(ImportedSemanticWorldBuildError::DuplicateProvider(
                pair[0].certificate().identity(),
            ));
        }
        Ok(seeds)
    }

    pub(super) const fn role(&self) -> ProviderSeedRole {
        match self {
            Self::TrustedCore(_) => ProviderSeedRole::TrustedCore,
            Self::Direct(_) => ProviderSeedRole::Direct,
            Self::Support(_) => ProviderSeedRole::Support,
        }
    }

    pub(super) const fn certificate(&self) -> &ImportedProviderCertificate {
        match self {
            Self::TrustedCore(input) => &input.certificate,
            Self::Direct(input) => &input.certificate,
            Self::Support(input) => &input.certificate,
        }
    }

    const fn foundation(&self) -> &'input ImportedHirFoundation {
        match self {
            Self::TrustedCore(input) => input.foundation,
            Self::Direct(input) => input.foundation,
            Self::Support(input) => input.foundation,
        }
    }

    fn validate(&self, current: ConeIdentity) -> Result<(), ImportedSemanticWorldBuildError> {
        let identity = self.certificate().identity();
        match self.role() {
            ProviderSeedRole::TrustedCore if identity != ConeIdentity::CORE => {
                return Err(ImportedSemanticWorldBuildError::TrustedProviderIsNotCore(
                    identity,
                ));
            }
            ProviderSeedRole::Direct | ProviderSeedRole::Support
                if identity == ConeIdentity::CORE =>
            {
                return Err(ImportedSemanticWorldBuildError::CoreUsedAsOrdinaryProvider);
            }
            ProviderSeedRole::TrustedCore
            | ProviderSeedRole::Direct
            | ProviderSeedRole::Support => {}
        }
        if identity == current {
            return Err(ImportedSemanticWorldBuildError::CurrentUsedAsProvider(
                current,
            ));
        }
        let derived = self.certificate().coordinate().identity().map_err(|_| {
            ImportedSemanticWorldBuildError::CoordinateIdentityUnavailable(identity)
        })?;
        if derived != identity {
            return Err(
                ImportedSemanticWorldBuildError::CoordinateIdentityMismatch {
                    declared: identity,
                    derived,
                },
            );
        }
        let foundation = self.foundation().origin();
        if foundation != identity {
            return Err(ImportedSemanticWorldBuildError::FoundationOriginMismatch {
                provider: identity,
                foundation,
            });
        }
        Ok(())
    }
}

pub(super) struct ImportedProvider<'input> {
    id: WorldConeId,
    role: ProviderSeedRole,
    certificate: ImportedProviderCertificate,
    foundation: &'input ImportedHirFoundation,
    interface: &'input CrossConeHirInterfaceSectionV1,
    alias_expansions: &'input CanonicalTypeAliasExpansionsV1,
    public_bindings: Vec<ImportedPublicBinding<'input>>,
    binding_positions: BTreeMap<PersistentExportBindingId, usize>,
    nested_bindings: BTreeSet<PersistentExportBindingId>,
}

impl<'input> ImportedProvider<'input> {
    pub(super) fn new(id: WorldConeId, seed: ProviderSeed<'input>) -> Self {
        let role = seed.role();
        let (certificate, foundation, interface, alias_expansions) = match seed {
            ProviderSeed::TrustedCore(input) => (
                input.certificate,
                input.foundation,
                input.interface,
                input.alias_expansions,
            ),
            ProviderSeed::Direct(input) => (
                input.certificate,
                input.foundation,
                input.interface,
                input.alias_expansions,
            ),
            ProviderSeed::Support(input) => (
                input.certificate,
                input.foundation,
                input.interface,
                input.alias_expansions,
            ),
        };
        let nested_bindings = interface
            .nominal_interfaces()
            .records()
            .iter()
            .flat_map(|record| record.nested_bindings().values().iter().copied())
            .collect();
        Self {
            id,
            role,
            certificate,
            foundation,
            interface,
            alias_expansions,
            public_bindings: Vec::new(),
            binding_positions: BTreeMap::new(),
            nested_bindings,
        }
    }

    pub(super) fn build_public_bindings(
        &mut self,
        entities: &ImportedEntityIndex,
    ) -> Result<(), ImportedSemanticWorldBuildError> {
        let provider = self.identity();
        let mut bindings = Vec::with_capacity(self.interface.public_bindings().records().len());
        let mut positions = BTreeMap::new();
        for record in self.interface.public_bindings().records() {
            let binding = record.binding();
            let identity = self.foundation.identity(binding).ok_or(
                ImportedSemanticWorldBuildError::MissingBindingIdentity { provider, binding },
            )?;
            let key = self
                .foundation
                .semantic_world_export_binding_key(binding)
                .ok_or(ImportedSemanticWorldBuildError::MissingBindingKey { provider, binding })?;
            if key.exporter() != provider {
                return Err(ImportedSemanticWorldBuildError::BindingExporterMismatch {
                    provider,
                    binding,
                    actual: key.exporter(),
                });
            }
            let target = entities.import_target(key.target()).ok_or(
                ImportedSemanticWorldBuildError::MissingBindingTarget {
                    provider,
                    binding,
                    target: key.target(),
                },
            )?;
            positions.insert(binding, bindings.len());
            bindings.push(ImportedPublicBinding {
                provider: self.id,
                identity,
                key,
                target,
                source: record.source(),
                lookup_sources: if self.is_direct() {
                    DirectDependencyImportSource::from_validated_binding(
                        self.id,
                        &self.certificate,
                        identity,
                        target,
                        record.source(),
                    )
                    .map_err(|error| {
                        ImportedSemanticWorldBuildError::InvalidLookupWitnessRoute {
                            provider,
                            binding,
                            error: Box::new(error),
                        }
                    })?
                } else {
                    Vec::new()
                },
            });
        }
        if let Some(binding) = self
            .nested_bindings
            .iter()
            .find(|binding| !positions.contains_key(binding))
        {
            return Err(ImportedSemanticWorldBuildError::MissingNestedBinding {
                provider,
                binding: *binding,
            });
        }
        self.public_bindings = bindings;
        self.binding_positions = positions;
        Ok(())
    }

    pub(super) const fn id(&self) -> WorldConeId {
        self.id
    }

    pub(super) const fn identity(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub(super) const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub(super) const fn foundation(&self) -> &'input ImportedHirFoundation {
        self.foundation
    }

    pub(super) const fn interface(&self) -> &'input CrossConeHirInterfaceSectionV1 {
        self.interface
    }

    pub(super) const fn alias_expansions(&self) -> &'input CanonicalTypeAliasExpansionsV1 {
        self.alias_expansions
    }

    pub(super) const fn is_direct(&self) -> bool {
        matches!(
            self.role,
            ProviderSeedRole::TrustedCore | ProviderSeedRole::Direct
        )
    }

    pub(super) const fn is_support(&self) -> bool {
        matches!(self.role, ProviderSeedRole::Support)
    }

    pub(super) fn is_package_binding(&self, binding: PersistentExportBindingId) -> bool {
        !self.nested_bindings.contains(&binding)
    }

    pub(super) fn public_bindings(&self) -> &[ImportedPublicBinding<'input>] {
        &self.public_bindings
    }

    pub(super) fn binding(&self, index: usize) -> Option<&ImportedPublicBinding<'input>> {
        self.public_bindings.get(index)
    }

    pub(super) fn binding_by_id(
        &self,
        binding: PersistentExportBindingId,
    ) -> Option<&ImportedPublicBinding<'input>> {
        self.binding_positions
            .get(&binding)
            .and_then(|index| self.public_bindings.get(*index))
    }
}
