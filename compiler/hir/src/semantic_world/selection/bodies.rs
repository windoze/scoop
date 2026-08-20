//! Provider implementations are independent of source name lookup entries.

use std::sync::Arc;

use super::{ImportedDependencyDefinitionSource, ImportedDependencyDefinitionSources};

#[derive(Clone, Debug)]
pub struct ImportedCallableBody {
    pub(super) body: Arc<crate::ExportGenericCallableBodyV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
    pub(super) source_declaration: Option<Arc<scoop_identity::SourceDeclarationKey>>,
}

impl ImportedCallableBody {
    pub fn source_location(
        &self,
        source: &scoop_identity::SourceIdentity,
        context: scoop_identity::PersistentSourceContextId,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_sources.resolve_location(source, context)
    }
    pub fn body(&self) -> &crate::ExportGenericCallableBodyV1 {
        &self.body
    }

    /// Named source functions have names; compiler-generated bodies do not.
    pub fn source_name(&self) -> Option<&scoop_identity::CanonicalIdentifier> {
        match self.source_declaration.as_ref()?.name() {
            scoop_identity::DeclarationName::Named(name) => Some(name),
            scoop_identity::DeclarationName::Constructor => None,
        }
    }

    pub fn lexical_parent(&self) -> Option<scoop_identity::CallableTemplateOwner> {
        use scoop_identity::{CallableTemplateOwner as Parent, DefinitionOwnerAtom as Owner};
        let declaration = self.source_declaration.as_ref()?;
        if !matches!(
            declaration.scope(),
            scoop_identity::DeclarationScope::LexicalScoped { .. }
        ) {
            return None;
        }
        Some(match declaration.owners().owners().last()? {
            Owner::Function(id) => Parent::Function(*id),
            Owner::GenericFunction(id) => Parent::GenericFunction(*id),
            Owner::Constructor(id) => Parent::Constructor(*id),
            Owner::PropertyAccessor(id) => Parent::Accessor(*id),
            Owner::GeneratedCallable(id) => Parent::Generated(*id),
            Owner::EnumVariant(id) => Parent::VariantConstructor(*id),
            Owner::Type(_)
            | Owner::GenericType(_)
            | Owner::Property(_)
            | Owner::ExtensionProperty(_) => return None,
        })
    }

    pub fn definition_source(
        &self,
        source: &crate::ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_sources.resolve(source)
    }
}

impl super::ImportedDependencySelectionPlan {
    pub fn derived_equality(
        &self,
        nominal: scoop_identity::PersistentTypeId,
    ) -> Option<crate::ImportedDerivedEquality> {
        use scoop_identity::{
            ExactTypeKey, GeneratedCallableKey, PersistentExactTypeId,
            PersistentGeneratedCallableId,
        };
        let declaration = self.nominal(nominal)?;
        let owner = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal))
            .expect("a checked nominal has an exact identity");
        let key = GeneratedCallableKey::DerivedEquality { exact_owner: owner };
        let callable = PersistentGeneratedCallableId::from_key(&key)
            .expect("a checked exact owner has a generated equality identity");
        self.generated_callable_definition(callable)?;
        Some(crate::ImportedDerivedEquality {
            provider: declaration.origin.origin().source().cone(),
            callable,
            owner,
        })
    }

    pub fn generated_callable_definition(
        &self,
        id: scoop_identity::PersistentGeneratedCallableId,
    ) -> Option<&crate::concrete::GeneratedCallableRecord> {
        self.catalog
            .generated_callables
            .get(&id)
            .map(|entry| &entry.definition)
    }

    pub fn callback_registration(
        &self,
        id: scoop_identity::PersistentCallbackRegistrationId,
    ) -> Option<(
        &crate::HirCallbackRegistrationIdentity,
        &scoop_identity::DefinitionOrigin,
    )> {
        self.catalog
            .callback_registrations
            .get(&id)
            .map(|(identity, origin)| (identity, origin))
    }

    pub fn generated_callable_definition_origin(
        &self,
        id: scoop_identity::PersistentGeneratedCallableId,
    ) -> Option<&crate::ExportDefinitionSourceV1> {
        self.catalog.generated_callables.get(&id)?.origin.as_ref()
    }

    pub fn generic_delegate(
        &self,
        property: scoop_identity::PersistentExtensionPropertyId,
    ) -> Option<Arc<crate::ExportGenericDelegateTemplateV1>> {
        self.catalog.delegates.get(&property).cloned()
    }

    pub fn callable_body(
        &self,
        owner: crate::DefaultCallableDeclarationV1,
    ) -> Option<ImportedCallableBody> {
        self.catalog.bodies.get(&owner).cloned()
    }
}
