//! Read-only imported semantic world used by ordinary cross-Cone HIR lookup.
//!
//! Provider roles are represented by distinct public view types. In
//! particular, a support provider has no API that enumerates names or public
//! bindings; it can only answer exact, kind-specific persistent-id queries.

use std::collections::BTreeMap;

use scoop_identity::ConeIdentity;

use crate::SourceNominalId;

mod entities;
mod error;
mod namespace;
mod native_boundary;
mod nominal_materialization;
mod nominal_signatures;
mod production_authority;
mod provider;
mod selection;
mod witness;

pub use entities::*;
pub use error::*;
pub use namespace::*;
pub use production_authority::*;
pub use provider::*;
pub use selection::*;
pub use witness::*;

use entities::ImportedEntityIndex;
use namespace::build_direct_package_index;
use provider::{ImportedProvider, ProviderSeed};

/// Immutable semantic projection of one validated dependency closure.
pub struct ImportedSemanticWorld<'input> {
    current: ConeIdentity,
    providers: Vec<ImportedProvider<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    direct: Vec<ConeIdentity>,
    support: Vec<ConeIdentity>,
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
        direct: Vec<DirectImportedProviderInput<'input>>,
        support: Vec<SupportImportedProviderInput<'input>>,
    ) -> Result<Self, ImportedSemanticWorldBuildError> {
        let seeds = ProviderSeed::canonicalize(current, direct, support)?;
        let mut providers = Vec::with_capacity(seeds.len());
        let mut positions = BTreeMap::new();
        let mut direct_ids = Vec::new();
        let mut support_ids = Vec::new();

        for (index, seed) in seeds.into_iter().enumerate() {
            let identity = seed.certificate().identity();
            positions.insert(identity, index);
            match seed.role() {
                provider::ProviderSeedRole::Direct => direct_ids.push(identity),
                provider::ProviderSeedRole::Support => support_ids.push(identity),
            }
            providers.push(ImportedProvider::new(seed));
        }

        direct_ids.sort_unstable();
        support_ids.sort_unstable();
        let entities = ImportedEntityIndex::build(&providers)?;
        for provider in &mut providers {
            provider.build_public_bindings(&entities)?;
        }
        let direct_packages = build_direct_package_index(&providers);

        Ok(Self {
            current,
            providers,
            positions,
            direct: direct_ids,
            support: support_ids,
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
            provider: &self.providers[self.positions[id]],
        })
    }

    pub fn support_providers(
        &self,
    ) -> impl ExactSizeIterator<Item = SupportProviderView<'_, 'input>> {
        self.support.iter().map(|id| SupportProviderView {
            provider: &self.providers[self.positions[id]],
        })
    }

    pub const fn direct_packages(&self) -> &DirectPackageIndex {
        &self.direct_packages
    }

    pub fn nominal(&self, declaration: SourceNominalId) -> Option<ImportedNominal<'input>> {
        let provider = self
            .entities
            .nominal_provider(declaration)
            .and_then(|id| self.provider(id))?;
        ImportedTypedProviderView { provider }.nominal(declaration)
    }

    pub fn callable(
        &self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Option<ImportedCallable<'input>> {
        let provider = self
            .entities
            .callable_provider(declaration)
            .and_then(|id| self.provider(id))?;
        ImportedTypedProviderView { provider }.callable(declaration)
    }

    pub fn property(
        &self,
        declaration: scoop_identity::PropertyOwner,
    ) -> Option<ImportedProperty<'input>> {
        let provider = self
            .entities
            .property_provider(declaration)
            .and_then(|id| self.provider(id))?;
        ImportedTypedProviderView { provider }.property(declaration)
    }

    pub fn type_alias(
        &self,
        alias: scoop_identity::PersistentTypeAliasId,
    ) -> Option<ImportedTypeAlias<'input>> {
        let provider = self
            .entities
            .alias_provider(alias)
            .and_then(|id| self.provider(id))?;
        ImportedTypedProviderView { provider }.type_alias(alias)
    }

    pub fn object_value(
        &self,
        value: scoop_identity::PersistentObjectValueId,
    ) -> Option<ImportedObjectValue<'input>> {
        let provider = self
            .entities
            .object_value_provider(value)
            .and_then(|id| self.provider(id))?;
        ImportedTypedProviderView { provider }.object_value(value)
    }

    pub fn enum_variant(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Option<ImportedEnumVariant<'input>> {
        let provider = self
            .entities
            .enum_variant_provider(variant)
            .and_then(|id| self.provider(id))?;
        ImportedTypedProviderView { provider }.enum_variant(variant)
    }

    fn provider(&self, identity: ConeIdentity) -> Option<&ImportedProvider<'input>> {
        self.positions
            .get(&identity)
            .map(|position| &self.providers[*position])
    }
}

#[cfg(test)]
mod tests;
