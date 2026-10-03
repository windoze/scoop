//! Consumer-side handles retain the provider's delegated property identity.

#[derive(Debug, Clone)]
pub struct ImportedGenericDelegateTemplate {
    pub property: scoop_identity::PersistentExtensionPropertyId,
    pub effective_type: crate::TypeId,
    pub initializer: crate::ImportedGenericCallableTemplateId,
    pub ensure: crate::ImportedGenericCallableTemplateId,
    pub diagnostic_path: String,
}
