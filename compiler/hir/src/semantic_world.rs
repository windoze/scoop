//! Read-only imported semantic world used by ordinary cross-Cone HIR lookup.
//!
//! Provider roles are represented by distinct public view types. In
//! particular, a support provider has no API that enumerates names or public
//! bindings; it can only answer exact, kind-specific persistent-id queries.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use scoop_identity::ConeIdentity;

use crate::SourceNominalId;

mod entities;
mod error;
mod namespace;
mod provider;

pub use entities::*;
pub use error::*;
pub use namespace::*;
pub use provider::*;

use entities::ImportedEntityIndex;
use namespace::build_direct_package_index;
use provider::{ImportedProvider, ProviderSeed};

static NEXT_WORLD_BRAND: AtomicU64 = AtomicU64::new(1);

/// Immutable semantic projection of one validated dependency closure.
pub struct ImportedSemanticWorld<'input> {
    brand: u64,
    current: ConeIdentity,
    providers: Vec<ImportedProvider<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    direct: Vec<WorldConeId>,
    support: Vec<WorldConeId>,
    trusted_core: Option<WorldConeId>,
    entities: ImportedEntityIndex,
    direct_packages: DirectPackageIndex,
}

impl<'input> ImportedSemanticWorld<'input> {
    /// Builds a world from the role-separated projections of a fully
    /// validated closure. This constructor is public only for the slib/driver
    /// boundary; ordinary lowering consumes the resulting read-only world.
    #[doc(hidden)]
    pub fn from_validated_closure(
        current: ConeIdentity,
        trusted_core: Option<TrustedCoreImportedProviderInput<'input>>,
        direct: Vec<DirectImportedProviderInput<'input>>,
        support: Vec<SupportImportedProviderInput<'input>>,
    ) -> Result<Self, ImportedSemanticWorldBuildError> {
        validate_core_role(current, trusted_core.as_ref(), &direct, &support)?;
        let brand = next_world_brand()?;
        let seeds = ProviderSeed::canonicalize(current, trusted_core, direct, support)?;
        let mut providers = Vec::with_capacity(seeds.len());
        let mut positions = BTreeMap::new();
        let mut direct_ids = Vec::new();
        let mut support_ids = Vec::new();
        let mut trusted_core_id = None;

        for (index, seed) in seeds.into_iter().enumerate() {
            let index = u32::try_from(index)
                .map_err(|_| ImportedSemanticWorldBuildError::ProviderCountOverflow)?;
            let id = WorldConeId::new(brand, index);
            let identity = seed.certificate().identity();
            positions.insert(identity, index as usize);
            match seed.role() {
                provider::ProviderSeedRole::TrustedCore => {
                    direct_ids.push(id);
                    trusted_core_id = Some(id);
                }
                provider::ProviderSeedRole::Direct => direct_ids.push(id),
                provider::ProviderSeedRole::Support => support_ids.push(id),
            }
            providers.push(ImportedProvider::new(id, seed));
        }

        direct_ids.sort_unstable_by_key(|id| providers[id.index()].identity());
        support_ids.sort_unstable_by_key(|id| providers[id.index()].identity());
        let entities = ImportedEntityIndex::build(&providers)?;
        for provider in &mut providers {
            provider.build_public_bindings(&entities)?;
        }
        let direct_packages = build_direct_package_index(brand, &providers, &direct_ids);

        Ok(Self {
            brand,
            current,
            providers,
            positions,
            direct: direct_ids,
            support: support_ids,
            trusted_core: trusted_core_id,
            entities,
            direct_packages,
        })
    }

    pub const fn current(&self) -> ConeIdentity {
        self.current
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn direct_provider_count(&self) -> usize {
        self.direct.len()
    }

    pub fn support_provider_count(&self) -> usize {
        self.support.len()
    }

    pub fn trusted_core(&self) -> Option<TrustedCoreProviderView<'_, 'input>> {
        self.trusted_core.map(|id| TrustedCoreProviderView {
            direct: DirectProviderView {
                provider: &self.providers[id.index()],
            },
        })
    }

    pub fn direct_provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<DirectProviderView<'_, 'input>> {
        let provider = self.provider(identity)?;
        provider
            .is_direct()
            .then_some(DirectProviderView { provider })
    }

    pub fn support_provider(
        &self,
        identity: ConeIdentity,
    ) -> Option<SupportProviderView<'_, 'input>> {
        let provider = self.provider(identity)?;
        provider
            .is_support()
            .then_some(SupportProviderView { provider })
    }

    pub fn direct_providers(
        &self,
    ) -> impl ExactSizeIterator<Item = DirectProviderView<'_, 'input>> {
        self.direct.iter().map(|id| DirectProviderView {
            provider: &self.providers[id.index()],
        })
    }

    pub fn support_providers(
        &self,
    ) -> impl ExactSizeIterator<Item = SupportProviderView<'_, 'input>> {
        self.support.iter().map(|id| SupportProviderView {
            provider: &self.providers[id.index()],
        })
    }

    pub const fn direct_packages(&self) -> &DirectPackageIndex {
        &self.direct_packages
    }

    pub fn nominal(&self, declaration: SourceNominalId) -> Option<ImportedNominal<'input>> {
        let provider = self
            .entities
            .nominal_provider(declaration)
            .and_then(|id| self.provider_by_id(id))?;
        ImportedTypedProviderView { provider }.nominal(declaration)
    }

    pub fn callable(
        &self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Option<ImportedCallable<'input>> {
        let provider = self
            .entities
            .callable_provider(declaration)
            .and_then(|id| self.provider_by_id(id))?;
        ImportedTypedProviderView { provider }.callable(declaration)
    }

    pub fn property(
        &self,
        declaration: scoop_identity::PropertyOwner,
    ) -> Option<ImportedProperty<'input>> {
        let provider = self
            .entities
            .property_provider(declaration)
            .and_then(|id| self.provider_by_id(id))?;
        ImportedTypedProviderView { provider }.property(declaration)
    }

    pub fn type_alias(
        &self,
        alias: scoop_identity::PersistentTypeAliasId,
    ) -> Option<ImportedTypeAlias<'input>> {
        let provider = self
            .entities
            .alias_provider(alias)
            .and_then(|id| self.provider_by_id(id))?;
        ImportedTypedProviderView { provider }.type_alias(alias)
    }

    pub fn object_value(
        &self,
        value: scoop_identity::PersistentObjectValueId,
    ) -> Option<ImportedObjectValue<'input>> {
        let provider = self
            .entities
            .object_value_provider(value)
            .and_then(|id| self.provider_by_id(id))?;
        ImportedTypedProviderView { provider }.object_value(value)
    }

    pub fn enum_variant(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Option<ImportedEnumVariant<'input>> {
        let provider = self
            .entities
            .enum_variant_provider(variant)
            .and_then(|id| self.provider_by_id(id))?;
        ImportedTypedProviderView { provider }.enum_variant(variant)
    }

    fn provider(&self, identity: ConeIdentity) -> Option<&ImportedProvider<'input>> {
        self.positions
            .get(&identity)
            .map(|position| &self.providers[*position])
    }

    fn provider_by_id(&self, id: WorldConeId) -> Option<&ImportedProvider<'input>> {
        (id.brand() == self.brand)
            .then(|| self.providers.get(id.index()))
            .flatten()
    }
}

fn validate_core_role(
    current: ConeIdentity,
    trusted_core: Option<&TrustedCoreImportedProviderInput<'_>>,
    direct: &[DirectImportedProviderInput<'_>],
    support: &[SupportImportedProviderInput<'_>],
) -> Result<(), ImportedSemanticWorldBuildError> {
    if current == ConeIdentity::CORE {
        if trusted_core.is_some() || !direct.is_empty() || !support.is_empty() {
            return Err(ImportedSemanticWorldBuildError::CoreCurrentHasProviders);
        }
        return Ok(());
    }
    if trusted_core.is_none() {
        return Err(ImportedSemanticWorldBuildError::MissingTrustedCore);
    }
    Ok(())
}

fn next_world_brand() -> Result<u64, ImportedSemanticWorldBuildError> {
    NEXT_WORLD_BRAND
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |brand| {
            brand.checked_add(1)
        })
        .map_err(|_| ImportedSemanticWorldBuildError::WorldBrandExhausted)
}

#[cfg(test)]
mod tests;
