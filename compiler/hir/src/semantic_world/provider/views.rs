use scoop_identity::{
    CallableTemplateOrigin, PersistentEnumVariantId, PersistentExportBindingId, PersistentId,
    PersistentObjectValueId, PersistentPropertyId, PersistentTypeAliasId, PropertyOwner,
};

use super::super::{import_callable_id, import_nominal_id, import_property_id};
use super::{ImportedProvider, ImportedProviderCertificate, WorldConeId};
use crate::{
    CallableSourceInterfaceV1, ExportBindingSourceV1, ExportConstValueV1,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ImportedCallable, ImportedEnumVariant,
    ImportedHirId, ImportedNominal, ImportedObjectValue, ImportedProperty, ImportedTarget,
    ImportedTypeAlias, NominalSourceShapeV1, SourceNominalId,
};

/// One imported public binding with both persistent and session-local
/// identities. The source remains the already validated declared/re-export
/// proof from the provider artifact.
#[derive(Clone, Copy)]
pub struct ImportedPublicBinding<'input> {
    pub(super) provider: WorldConeId,
    pub(super) identity: ImportedHirId<PersistentExportBindingId>,
    pub(super) key: &'input scoop_identity::ExportBindingKey,
    pub(super) target: ImportedTarget,
    pub(super) source: &'input ExportBindingSourceV1,
}

impl<'input> ImportedPublicBinding<'input> {
    pub const fn provider(self) -> WorldConeId {
        self.provider
    }

    pub const fn identity(self) -> ImportedHirId<PersistentExportBindingId> {
        self.identity
    }

    pub const fn key(self) -> &'input scoop_identity::ExportBindingKey {
        self.key
    }

    pub const fn target(self) -> ImportedTarget {
        self.target
    }

    pub const fn source(self) -> &'input ExportBindingSourceV1 {
        self.source
    }
}

/// Exact typed lookup view shared by direct and support providers.
#[derive(Clone, Copy)]
pub struct ImportedTypedProviderView<'world, 'input> {
    pub(in crate::semantic_world) provider: &'world ImportedProvider<'input>,
}

impl<'world, 'input> ImportedTypedProviderView<'world, 'input> {
    pub const fn id(self) -> WorldConeId {
        self.provider.id()
    }

    pub const fn certificate(self) -> &'world ImportedProviderCertificate {
        self.provider.certificate()
    }

    pub fn hir_identity<I: PersistentId + 'static>(self, id: I) -> Option<ImportedHirId<I>> {
        self.provider.foundation().identity(id)
    }

    pub fn nominal(self, declaration: SourceNominalId) -> Option<ImportedNominal<'input>> {
        let record = self
            .provider
            .interface()
            .nominal_interfaces()
            .get(declaration)?;
        Some(ImportedNominal {
            provider: self.id(),
            declaration: import_nominal_id(self.provider, declaration)?,
            record,
        })
    }

    pub fn callable(self, declaration: CallableTemplateOrigin) -> Option<ImportedCallable<'input>> {
        let record = self
            .provider
            .interface()
            .callable_interfaces()
            .get(declaration)?;
        Some(ImportedCallable {
            provider: self.id(),
            declaration: import_callable_id(self.provider, declaration)?,
            record,
        })
    }

    pub fn property(self, declaration: PropertyOwner) -> Option<ImportedProperty<'input>> {
        let record = self
            .provider
            .interface()
            .property_interfaces()
            .get(declaration)?;
        Some(ImportedProperty {
            provider: self.id(),
            declaration: import_property_id(self.provider, declaration)?,
            record,
        })
    }

    pub fn type_alias(self, alias: PersistentTypeAliasId) -> Option<ImportedTypeAlias<'input>> {
        Some(ImportedTypeAlias {
            provider: self.id(),
            identity: self.provider.foundation().identity(alias)?,
            record: self.provider.interface().type_aliases().get(alias)?,
            expansion: self.provider.alias_expansions().get(alias)?,
        })
    }

    pub fn object_value(
        self,
        value: PersistentObjectValueId,
    ) -> Option<ImportedObjectValue<'input>> {
        self.provider
            .interface()
            .nominal_interfaces()
            .records()
            .iter()
            .find_map(|record| {
                let NominalSourceShapeV1::Object(shape) = record.source_shape() else {
                    return None;
                };
                if shape.value() != value {
                    return None;
                }
                Some(ImportedObjectValue {
                    provider: self.id(),
                    identity: self.provider.foundation().identity(value)?,
                    owner: import_nominal_id(self.provider, record.declaration())?,
                    owner_record: record,
                })
            })
    }

    pub fn enum_variant(
        self,
        variant: PersistentEnumVariantId,
    ) -> Option<ImportedEnumVariant<'input>> {
        self.provider
            .interface()
            .nominal_interfaces()
            .records()
            .iter()
            .find_map(|owner_record| {
                let NominalSourceShapeV1::Enum(shape) = owner_record.source_shape() else {
                    return None;
                };
                let record = shape
                    .variants()
                    .iter()
                    .find(|record| record.variant() == variant)?;
                Some(ImportedEnumVariant {
                    provider: self.id(),
                    identity: self.provider.foundation().identity(variant)?,
                    owner: import_nominal_id(self.provider, owner_record.declaration())?,
                    owner_record,
                    record,
                })
            })
    }

    pub fn callable_source(
        self,
        declaration: CallableTemplateOrigin,
    ) -> Option<&'input CallableSourceInterfaceV1> {
        self.provider
            .interface()
            .source_interfaces()
            .get(declaration)
    }

    pub fn default_template(
        self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Option<&'input ExportDefaultTemplateV1> {
        self.provider.interface().default_templates().get(key)
    }

    pub fn constant(self, property: PersistentPropertyId) -> Option<&'input ExportConstValueV1> {
        self.provider.interface().constants().get(property)
    }
}

/// Enumeration-capable view available only for a direct dependency.
#[derive(Clone, Copy)]
pub struct DirectProviderView<'world, 'input> {
    pub(in crate::semantic_world) provider: &'world ImportedProvider<'input>,
}

impl<'world, 'input> DirectProviderView<'world, 'input> {
    pub const fn id(self) -> WorldConeId {
        self.provider.id()
    }

    pub const fn certificate(self) -> &'world ImportedProviderCertificate {
        self.provider.certificate()
    }

    pub const fn typed(self) -> ImportedTypedProviderView<'world, 'input> {
        ImportedTypedProviderView {
            provider: self.provider,
        }
    }

    pub fn public_bindings(self) -> &'world [ImportedPublicBinding<'input>] {
        self.provider.public_bindings()
    }
}

/// Typed exact-lookup-only view of a transitive support dependency.
#[derive(Clone, Copy)]
pub struct SupportProviderView<'world, 'input> {
    pub(in crate::semantic_world) provider: &'world ImportedProvider<'input>,
}

impl<'world, 'input> SupportProviderView<'world, 'input> {
    pub const fn id(self) -> WorldConeId {
        self.provider.id()
    }

    pub const fn certificate(self) -> &'world ImportedProviderCertificate {
        self.provider.certificate()
    }

    pub const fn typed(self) -> ImportedTypedProviderView<'world, 'input> {
        ImportedTypedProviderView {
            provider: self.provider,
        }
    }
}

/// Direct core view carrying the additional trusted-core role proof.
#[derive(Clone, Copy)]
pub struct TrustedCoreProviderView<'world, 'input> {
    pub(in crate::semantic_world) direct: DirectProviderView<'world, 'input>,
}

impl<'world, 'input> TrustedCoreProviderView<'world, 'input> {
    pub const fn direct(self) -> DirectProviderView<'world, 'input> {
        self.direct
    }

    pub const fn certificate(self) -> &'world ImportedProviderCertificate {
        self.direct.certificate()
    }
}
