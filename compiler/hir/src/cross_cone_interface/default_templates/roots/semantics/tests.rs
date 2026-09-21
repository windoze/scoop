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
    let mut authority = Authority::new(1, 2);

    assert_eq!(
        root.validate_semantics(
            key,
            &path,
            &CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
            &mut authority
        ),
        Ok(DefaultTemplateProviderShapeV1::try_new(1, 2).unwrap())
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
    let mut accepted = Authority::new(1, 1);

    assert_eq!(
        provider.validate_semantics(
            key,
            &path,
            &CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
            &mut accepted
        ),
        Ok(DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap())
    );
    assert_eq!(
        accepted.inherited_calls,
        vec![(key, provider, path.clone())]
    );

    let mut rejected = Authority::new(1, 1);
    rejected.fail_relation = true;
    assert_eq!(
        provider.validate_semantics(
            key,
            &path,
            &CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
            &mut rejected
        ),
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
    let mut authority = Authority::new(0, 0);

    assert_eq!(
        root.validate_semantics(
            key,
            &invalid_path,
            &CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
            &mut authority
        ),
        Err(
            DefaultTemplateRootSemanticValidationError::InvalidDefinitionPathRole {
                actual: StructuralDefinitionSiteRole::Lambda,
            }
        )
    );
    assert!(authority.provider_calls.is_empty());

    authority.fail_provider = true;
    assert_eq!(
        root.validate_semantics(
            key,
            &default_path(0),
            &CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
            &mut authority
        ),
        Err(DefaultTemplateRootSemanticValidationError::Provider(
            AuthorityError::Provider
        ))
    );
}

#[test]
fn property_accessor_cannot_own_a_template() {
    let root = PersistentLexicalRootV1::Function(function("provider").id());
    let key = ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Accessor(accessor()), 0);
    let mut authority = Authority::new(0, 0);

    assert_eq!(
        root.validate_semantics(
            key,
            &default_path(0),
            &CanonicalBinderUseListV1::try_new(vec![]).unwrap(),
            &mut authority
        ),
        Err(DefaultTemplateRootSemanticValidationError::PropertyAccessorOwner)
    );
    assert!(authority.provider_calls.is_empty());
}

#[test]
fn provider_shape_preserves_frames_and_rejects_total_overflow() {
    let shape = DefaultTemplateProviderShapeV1::try_new(2, 3).unwrap();
    assert_eq!(shape.nominal_owner_binder_arity(), 2);
    assert_eq!(shape.callable_own_binder_arity(), 3);
    assert_eq!(shape.binder_arity(), 5);
    assert_eq!(shape.signature_scope().arity_at_depth(0), Some(3));
    assert_eq!(shape.signature_scope().arity_at_depth(1), Some(2));

    assert_eq!(
        DefaultTemplateProviderShapeV1::try_new(u32::MAX, 1),
        Err(
            DefaultTemplateProviderShapeBuildError::BinderArityOverflow {
                nominal_owner: u32::MAX,
                callable_own: 1,
            }
        )
    );
}

struct Authority {
    shape: DefaultTemplateProviderShapeV1,
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
    fn new(nominal_owner_binder_arity: u32, callable_own_binder_arity: u32) -> Self {
        Self {
            shape: DefaultTemplateProviderShapeV1::try_new(
                nominal_owner_binder_arity,
                callable_own_binder_arity,
            )
            .unwrap(),
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
            Ok(self.shape)
        }
    }

    fn default_template_provider_receiver(
        &mut self,
        _root: crate::PersistentLexicalRootV1,
        _path: &StructuralDefinitionPath,
    ) -> Result<Option<SignatureTypeKey>, AuthorityError> {
        Ok(None)
    }

    fn validate_inherited_default_provider(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
        root: PersistentLexicalRootV1,
        path: &StructuralDefinitionPath,
        _mapping: &crate::CanonicalBinderUseListV1,
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

#[test]
fn identity_mapping_preserves_host_and_own_binder_frames() {
    for (host, own, expected) in [
        (0, 0, vec![]),
        (0, 2, vec![(0, 0), (0, 1)]),
        (2, 0, vec![(0, 0), (0, 1)]),
        (2, 1, vec![(1, 0), (1, 1), (0, 0)]),
    ] {
        let shape = DefaultTemplateProviderShapeV1::try_new(host, own).unwrap();
        for (position, (depth, index)) in expected.into_iter().enumerate() {
            assert_eq!(
                shape.identity_binder_at(position as u32),
                Some(SignatureTypeKey::Binder { depth, index })
            );
        }
        assert_eq!(shape.identity_binder_at(host + own), None);
    }
}
