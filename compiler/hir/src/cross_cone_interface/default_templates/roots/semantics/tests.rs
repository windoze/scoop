use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, PackagePath, PersistentFunctionId,
    PersistentPropertyAccessorId, PropertyAccessorKey, PropertyOwner, SourceDeclarationKey,
    SourceDeclarationSite, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

use super::*;

#[test]
fn direct_provider_uses_typed_identity_equality() {
    let function = function("direct");
    let root = PersistentLexicalRootV1::Function(function.id());
    let key = ExportDefaultTemplateKeyV1::new(root.declaration(), 1);
    let path = default_path(0);
    let mut authority = Authority::new(3);

    assert_eq!(
        root.validate_semantics(key, &path, &mut authority),
        Ok(DefaultTemplateProviderShapeV1::new(3))
    );
    assert_eq!(authority.provider_calls, vec![(root, path)]);
    assert!(authority.inherited_calls.is_empty());
}

#[test]
fn inherited_provider_requires_an_explicit_relation() {
    let provider = PersistentLexicalRootV1::Function(function("base").id());
    let key = ExportDefaultTemplateKeyV1::new(
        CallableTemplateOrigin::Function(function("override").id()),
        0,
    );
    let path = default_path(1);
    let mut accepted = Authority::new(2);

    assert_eq!(
        provider.validate_semantics(key, &path, &mut accepted),
        Ok(DefaultTemplateProviderShapeV1::new(2))
    );
    assert_eq!(
        accepted.inherited_calls,
        vec![(key, provider, path.clone())]
    );

    let mut rejected = Authority::new(2);
    rejected.fail_relation = true;
    assert_eq!(
        provider.validate_semantics(key, &path, &mut rejected),
        Err(
            DefaultTemplateRootSemanticValidationError::InheritedRelation(AuthorityError::Relation)
        )
    );
}

#[test]
fn provider_authority_and_definition_path_fail_closed() {
    let function = function("provider");
    let root = PersistentLexicalRootV1::Function(function.id());
    let key = ExportDefaultTemplateKeyV1::new(root.declaration(), 0);
    let invalid_path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
        [],
    );
    let mut authority = Authority::new(0);

    assert_eq!(
        root.validate_semantics(key, &invalid_path, &mut authority),
        Err(
            DefaultTemplateRootSemanticValidationError::InvalidDefinitionPathRole {
                actual: StructuralDefinitionSiteRole::Lambda,
            }
        )
    );
    assert!(authority.provider_calls.is_empty());

    authority.fail_provider = true;
    assert_eq!(
        root.validate_semantics(key, &default_path(0), &mut authority),
        Err(DefaultTemplateRootSemanticValidationError::Provider(
            AuthorityError::Provider
        ))
    );
}

#[test]
fn property_accessor_cannot_own_a_template() {
    let root = PersistentLexicalRootV1::Function(function("provider").id());
    let key = ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Accessor(accessor()), 0);
    let mut authority = Authority::new(0);

    assert_eq!(
        root.validate_semantics(key, &default_path(0), &mut authority),
        Err(DefaultTemplateRootSemanticValidationError::PropertyAccessorOwner)
    );
    assert!(authority.provider_calls.is_empty());
}

struct Authority {
    binder_arity: u32,
    fail_provider: bool,
    fail_relation: bool,
    provider_calls: Vec<(PersistentLexicalRootV1, StructuralDefinitionPath)>,
    inherited_calls: Vec<(
        ExportDefaultTemplateKeyV1,
        PersistentLexicalRootV1,
        StructuralDefinitionPath,
    )>,
}

impl Authority {
    const fn new(binder_arity: u32) -> Self {
        Self {
            binder_arity,
            fail_provider: false,
            fail_relation: false,
            provider_calls: Vec::new(),
            inherited_calls: Vec::new(),
        }
    }
}

impl DefaultTemplateRootSemanticAuthority<AuthorityError> for Authority {
    fn default_template_provider_shape(
        &mut self,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<DefaultTemplateProviderShapeV1, AuthorityError> {
        self.provider_calls.push((root, path.clone()));
        if self.fail_provider {
            Err(AuthorityError::Provider)
        } else {
            Ok(DefaultTemplateProviderShapeV1::new(self.binder_arity))
        }
    }

    fn validate_inherited_default_provider(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
    ) -> Result<(), AuthorityError> {
        self.inherited_calls.push((key, root, path.clone()));
        if self.fail_relation {
            Err(AuthorityError::Relation)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthorityError {
    Provider,
    Relation,
}

impl std::fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AuthorityError {}

fn default_path(ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, ordinal),
        [],
    )
}

fn function(name: &str) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn accessor() -> PersistentPropertyAccessorId {
    let property = CborIdentityRecord::from_key(SourceDeclarationKey::property(
        site(),
        CanonicalIdentifier::new("value").unwrap(),
    ))
    .unwrap();
    PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        PropertyOwner::Property(property.id()),
        AccessorRole::Getter,
    ))
    .unwrap()
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
