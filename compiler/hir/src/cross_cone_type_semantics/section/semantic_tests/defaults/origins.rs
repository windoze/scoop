use super::*;

impl ProtectedDefaultRootSemanticAuthority<&'static str> for DefaultAuthority {
    fn protected_default_provider_shape(
        &mut self,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, &'static str> {
        Err("fixture has no default template")
    }
    fn protected_default_provider_parameter(
        &mut self,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<crate::DefaultTemplateProviderParameterV1<'_>, &'static str> {
        Err("fixture has no default template")
    }

    fn protected_default_provider_receiver(
        &mut self,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<Option<SignatureTypeKey>, &'static str> {
        Err("fixture has no default template")
    }
    fn validate_inherited_protected_default_provider(
        &mut self,
        _key: ProtectedDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _mapping: &CanonicalBinderUseListV1,
    ) -> Result<(), &'static str> {
        Err("fixture has no default template")
    }
}

impl ProtectedDefaultOriginSemanticAuthority<&'static str> for DefaultAuthority {
    fn validate_protected_default_origin(
        &mut self,
        _key: ProtectedDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        Err("fixture has no default template")
    }
    fn validate_protected_default_local_origin(
        &mut self,
        _key: ProtectedDefaultTemplateKeyV1,
        _root: PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
        _selector: &LocalValueSelector,
        _origin: &ExportDefinitionSourceV1,
    ) -> Result<(), &'static str> {
        Err("fixture has no default template")
    }
}
