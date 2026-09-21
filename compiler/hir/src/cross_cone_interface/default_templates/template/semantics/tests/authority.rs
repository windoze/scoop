use super::*;

pub(super) struct Authority {
    pub(super) declaration: CallableTemplateOrigin,
    pub(super) identity: CallableDeclarationIdentityShapeV1,
    pub(super) provider: PersistentLexicalRootV1,
    pub(super) definition_path: StructuralDefinitionPath,
    pub(super) nominals: Vec<(crate::SourceNominalId, PublicNominalShapeV1)>,
    pub(super) provider_receiver: Option<SignatureTypeKey>,
    pub(super) provider_shape: DefaultTemplateProviderShapeV1,
    pub(super) expected_mapping: Option<CanonicalBinderUseListV1>,
    pub(super) inherited_validations: usize,
    pub(super) definition_source_validations: usize,
    pub(super) root_origin_validations: usize,
    pub(super) local_origin_validations: usize,
    pub(super) reject_definition_source: bool,
    pub(super) reject_root_origin: bool,
    pub(super) reject_local_origin: bool,
}

impl NominalInterfaceShapeAuthority<AuthorityError> for Authority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        self.nominals
            .iter()
            .find(|(owner, _)| *owner == crate::SourceNominalId::Concrete(declaration))
            .map(|(_, shape)| *shape)
            .ok_or(AuthorityError::ConcreteNominal(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, AuthorityError> {
        self.nominals
            .iter()
            .find(|(owner, _)| *owner == crate::SourceNominalId::GenericTemplate(declaration))
            .map(|(_, shape)| *shape)
            .ok_or(AuthorityError::GenericNominal(declaration))
    }
}

impl CallableInterfaceSemanticAuthority<AuthorityError> for Authority {
    fn callable_declaration_identity_shape(
        &mut self,
        declaration: CallableTemplateOrigin,
    ) -> Result<CallableDeclarationIdentityShapeV1, AuthorityError> {
        if declaration == self.declaration {
            Ok(self.identity.clone())
        } else {
            Err(AuthorityError::Callable(declaration))
        }
    }
}

impl DefaultTemplateRootSemanticAuthority<AuthorityError> for Authority {
    fn default_template_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, AuthorityError> {
        if root != self.provider || path != &self.definition_path {
            return Err(AuthorityError::Provider);
        }
        Ok(self.provider_shape)
    }

    fn default_template_provider_receiver(
        &mut self,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<Option<SignatureTypeKey>, AuthorityError> {
        Ok(self.provider_receiver.clone())
    }

    fn validate_inherited_default_provider(
        &mut self,
        _key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        _mapping: &crate::CanonicalBinderUseListV1,
    ) -> Result<(), AuthorityError> {
        if root != self.provider || path != &self.definition_path {
            return Err(AuthorityError::Inherited);
        }
        if self
            .expected_mapping
            .as_ref()
            .is_some_and(|expected| expected != _mapping)
        {
            return Err(AuthorityError::Inherited);
        }
        self.inherited_validations += 1;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthorityError {
    Callable(CallableTemplateOrigin),
    ConcreteNominal(PersistentTypeId),
    GenericNominal(PersistentGenericTypeId),
    Provider,
    Inherited,
    DefinitionSource,
    RootOrigin,
    LocalOrigin,
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing default-contract authority: {self:?}")
    }
}

impl std::error::Error for AuthorityError {}
