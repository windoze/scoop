use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    NonEmptyVec, PackagePath, PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use super::*;
use crate::{CanonicalSignatureTypesV1, NominalTypeParameterBoundsV1, TypeParameterBinderV1};

#[test]
fn validates_bound_kind_arity_and_nested_binder_scope() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    let class = nominal_application(fixture.generic_class.id(), binder(0, 0));
    let interface = nominal_application(fixture.generic_interface.id(), binder(1, 0));
    let binders = binder_list(class, vec![interface]);

    assert_eq!(
        binders.validate_bound_semantics(Some(1), &mut authority),
        Ok(())
    );
}

#[test]
fn rejects_non_nominal_bound_roots() {
    let fixture = Fixture::new();
    let mut authority = fixture.authority();
    let binders = binder_list(binder(0, 0), Vec::new());

    assert!(matches!(
        binders.validate_bound_semantics(None, &mut authority),
        Err(TypeParameterBinderSemanticValidationError {
            binder_index: 0,
            bound: TypeParameterBoundLocation::Class,
            error: NominalBoundSemanticError::NonNominal {
                actual: SignatureTypeFormV1::Binder,
            },
        })
    ));
}

#[test]
fn rejects_class_and_interface_kind_mismatches() {
    let fixture = Fixture::new();
    let mut class_authority = fixture.authority();
    let class_is_interface = binder_list(
        SignatureTypeKey::Nominal(fixture.plain_interface.id()),
        Vec::new(),
    );
    assert!(matches!(
        class_is_interface.validate_bound_semantics(None, &mut class_authority),
        Err(TypeParameterBinderSemanticValidationError {
            bound: TypeParameterBoundLocation::Class,
            error: NominalBoundSemanticError::Kind {
                expected: PublicNominalKindV1::Class,
                actual: PublicNominalKindV1::Interface,
            },
            ..
        })
    ));

    let mut interface_authority = fixture.authority();
    let interface_is_class = binder_list(
        SignatureTypeKey::Nominal(fixture.plain_class.id()),
        vec![SignatureTypeKey::Nominal(fixture.plain_class.id())],
    );
    assert!(matches!(
        interface_is_class.validate_bound_semantics(None, &mut interface_authority),
        Err(TypeParameterBinderSemanticValidationError {
            bound: TypeParameterBoundLocation::Interface { interface_index: 0 },
            error: NominalBoundSemanticError::Kind {
                expected: PublicNominalKindV1::Interface,
                actual: PublicNominalKindV1::Class,
            },
            ..
        })
    ));
}

#[test]
fn rejects_reference_form_arity_mismatches() {
    let fixture = Fixture::new();
    let mut concrete_authority = fixture.authority();
    concrete_authority.concrete.insert(
        fixture.plain_class.id(),
        PublicNominalShapeV1::new(PublicNominalKindV1::Class, 1),
    );
    let concrete = binder_list(
        SignatureTypeKey::Nominal(fixture.plain_class.id()),
        Vec::new(),
    );
    assert!(matches!(
        concrete.validate_bound_semantics(None, &mut concrete_authority),
        Err(TypeParameterBinderSemanticValidationError {
            error: NominalBoundSemanticError::Signature(
                SignatureTypeSemanticError::ConcreteNominalArity { actual: 1, .. }
            ),
            ..
        })
    ));

    let mut generic_authority = fixture.authority();
    let generic = binder_list(
        nominal_application(fixture.generic_class.id(), binder(0, 0)),
        Vec::new(),
    );
    generic_authority.generic.insert(
        fixture.generic_class.id(),
        PublicNominalShapeV1::new(PublicNominalKindV1::Class, 2),
    );
    assert!(matches!(
        generic.validate_bound_semantics(None, &mut generic_authority),
        Err(TypeParameterBinderSemanticValidationError {
            error: NominalBoundSemanticError::Signature(
                SignatureTypeSemanticError::GenericNominalArity {
                    expected: 2,
                    actual: 1,
                    ..
                }
            ),
            ..
        })
    ));
}

#[test]
fn recursively_rejects_missing_nominal_authority() {
    let fixture = Fixture::new();
    let missing = nominal("Missing", SourceNominalKind::Class, 0);
    let signature = nominal_application(
        fixture.generic_interface.id(),
        SignatureTypeKey::Nominal(missing.id()),
    );
    let scope = SignatureBinderScopeV1::for_declaration(0, None);
    let mut authority = fixture.authority();

    assert!(matches!(
        scope.validate_signature_semantics(&signature, &mut authority),
        Err(SignatureTypeSemanticError::Reference(TestAuthorityError::MissingConcrete(
            id
        ))) if id == missing.id()
    ));
}

#[test]
fn recursively_rejects_out_of_scope_application_arguments() {
    let fixture = Fixture::new();
    let signature = nominal_application(fixture.generic_interface.id(), binder(0, 1));
    let scope = SignatureBinderScopeV1::for_declaration(1, None);
    let mut authority = fixture.authority();

    assert!(matches!(
        scope.validate_signature_semantics(&signature, &mut authority),
        Err(SignatureTypeSemanticError::BinderScope(
            SignatureBinderScopeError::IndexOutOfRange {
                depth: 0,
                index: 1,
                arity: 1,
            }
        ))
    ));
}

struct Fixture {
    plain_class: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    plain_interface: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    generic_class: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>,
    generic_interface: CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            plain_class: nominal("PlainClass", SourceNominalKind::Class, 0),
            plain_interface: nominal("PlainInterface", SourceNominalKind::Interface, 0),
            generic_class: nominal("GenericClass", SourceNominalKind::Class, 1),
            generic_interface: nominal("GenericInterface", SourceNominalKind::Interface, 1),
        }
    }

    fn authority(&self) -> TestAuthority {
        TestAuthority {
            concrete: BTreeMap::from([
                (
                    self.plain_class.id(),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Class, 0),
                ),
                (
                    self.plain_interface.id(),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Interface, 0),
                ),
            ]),
            generic: BTreeMap::from([
                (
                    self.generic_class.id(),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Class, 1),
                ),
                (
                    self.generic_interface.id(),
                    PublicNominalShapeV1::new(PublicNominalKindV1::Interface, 1),
                ),
            ]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestAuthorityError {
    MissingConcrete(PersistentTypeId),
    MissingGeneric(PersistentGenericTypeId),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test nominal shape: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

struct TestAuthority {
    concrete: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
    generic: BTreeMap<PersistentGenericTypeId, PublicNominalShapeV1>,
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for TestAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.concrete
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::MissingConcrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.generic
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::MissingGeneric(declaration))
    }
}

fn binder_list(
    class: SignatureTypeKey,
    interfaces: Vec<SignatureTypeKey>,
) -> CanonicalBinderListV1 {
    let bounds = NominalTypeParameterBoundsV1::try_new(
        Some(class),
        CanonicalSignatureTypesV1::try_new(interfaces).unwrap(),
    )
    .unwrap();
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        CanonicalIdentifier::new("T").unwrap(),
        TypeParameterBoundsV1::Nominal(bounds),
    )])
    .unwrap()
}

fn nominal_application(
    origin: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::new(vec![argument]).unwrap(),
    }
}

fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}

fn nominal<I>(
    name: &str,
    kind: SourceNominalKind,
    arity: u32,
) -> CborIdentityRecord<I, SourceDeclarationKey>
where
    I: scoop_identity::PersistentId,
    SourceDeclarationKey: scoop_identity::CborIdentityKey<I>,
    <SourceDeclarationKey as scoop_identity::CborIdentityKey<I>>::Error: std::fmt::Debug,
{
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        arity,
    ))
    .unwrap()
}
