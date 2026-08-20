use scoop_identity::{CallableTemplateOrigin, CanonicalIdentifier, DeclarationName, GcEffect};

use super::{ImportedDependencySelectionPlan, ImportedDependencySelectionPlanBuildError};
use crate::semantic_world::ImportedProvider;
use crate::{
    CallableDeclarationRecordV1, CallableImplementationV1, CallableSourceInterfaceV1,
    IntrinsicFunctionKind,
};

#[derive(Clone, Debug)]
pub(super) enum CallableCatalogName {
    Function(CanonicalIdentifier),
    Constructor,
    Accessor,
    VariantConstructor(CanonicalIdentifier),
}

pub(super) fn callable_catalog_name(
    provider: &ImportedProvider<'_>,
    declaration: CallableTemplateOrigin,
) -> Result<CallableCatalogName, ImportedDependencySelectionPlanBuildError> {
    let foundation = provider.foundation().canonical_for_semantic_authority();
    let key = match declaration {
        CallableTemplateOrigin::Function(id) => foundation
            .function_by_bytes(id.as_array())
            .map(|(_, key)| key),
        CallableTemplateOrigin::GenericFunction(id) => foundation
            .generic_function_by_bytes(id.as_array())
            .map(|(_, key)| key),
        CallableTemplateOrigin::Constructor(_) => return Ok(CallableCatalogName::Constructor),
        CallableTemplateOrigin::Accessor(_) => return Ok(CallableCatalogName::Accessor),
        CallableTemplateOrigin::VariantConstructor(id) => {
            let name = foundation
                .enum_variant_by_bytes(id.as_array())
                .and_then(|(_, key)| key.source_name())
                .ok_or(
                    ImportedDependencySelectionPlanBuildError::MissingCallableSourceName(
                        declaration,
                    ),
                )?;
            return Ok(CallableCatalogName::VariantConstructor(name.clone()));
        }
    };
    match key.map(|key| key.name()) {
        Some(DeclarationName::Named(name)) => Ok(CallableCatalogName::Function(name.clone())),
        Some(DeclarationName::Constructor) | None => {
            Err(ImportedDependencySelectionPlanBuildError::MissingCallableSourceName(declaration))
        }
    }
}

pub(super) fn property_catalog_name(
    provider: &ImportedProvider<'_>,
    declaration: scoop_identity::PropertyOwner,
) -> Result<CanonicalIdentifier, ImportedDependencySelectionPlanBuildError> {
    let foundation = provider.foundation().canonical_for_semantic_authority();
    let key = match declaration {
        scoop_identity::PropertyOwner::Property(id) => foundation
            .property_by_bytes(id.as_array())
            .map(|(_, key)| key),
        scoop_identity::PropertyOwner::ExtensionProperty(id) => foundation
            .extension_property_by_bytes(id.as_array())
            .map(|(_, key)| key),
    };
    match key.map(|key| key.name()) {
        Some(DeclarationName::Named(name)) => Ok(name.clone()),
        _ => Err(ImportedDependencySelectionPlanBuildError::MissingPropertySourceName(declaration)),
    }
}

/// A complete source-call view of one shared intrinsic declaration.
#[derive(Clone, Copy)]
pub struct ImportedIntrinsicCallable<'a> {
    name: &'a CanonicalIdentifier,
    interface: &'a CallableDeclarationRecordV1,
    source: &'a CallableSourceInterfaceV1,
}

impl<'a> ImportedIntrinsicCallable<'a> {
    pub const fn name(self) -> &'a CanonicalIdentifier {
        self.name
    }

    pub const fn interface(self) -> &'a CallableDeclarationRecordV1 {
        self.interface
    }

    pub const fn source(self) -> &'a CallableSourceInterfaceV1 {
        self.source
    }
}

impl ImportedDependencySelectionPlan {
    pub fn intrinsic_callable(
        &self,
        kind: IntrinsicFunctionKind,
        gc_effect: GcEffect,
    ) -> Option<ImportedIntrinsicCallable<'_>> {
        let mut matches = self.catalog.callables.values().filter(|entry| {
            let effects = entry.interface.effects();
            effects.implementation() == CallableImplementationV1::Intrinsic(kind)
                && effects.gc_effect() == gc_effect
        });
        let entry = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let CallableCatalogName::Function(name) = &entry.name else {
            return None;
        };
        Some(ImportedIntrinsicCallable {
            name,
            interface: &entry.interface,
            source: entry.source.as_ref()?,
        })
    }
}
