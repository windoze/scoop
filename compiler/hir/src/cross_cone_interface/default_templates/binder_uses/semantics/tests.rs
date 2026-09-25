use scoop_identity::{
    CallingConvention, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionOwnerChain, Effect, NonEmptyVec, PackagePath, PersistentGenericTypeId,
    PersistentTypeId, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceNominalKind,
};

use super::*;
use crate::{DefaultTemplateProviderShapeV1, PublicNominalShapeV1, SignatureBinderScopeError};

#[test]
fn validates_exact_provider_arity_and_key_owner_scope() {
    let uses = CanonicalBinderUseListV1::try_new(vec![binder(1, 0), binder(0, 1)]).unwrap();
    let scope = SignatureBinderScopeV1::for_declaration(2, Some(1));

    assert_eq!(uses.validate_semantics(2, &scope, &mut NoNominals), Ok(()));
}

#[test]
fn rejects_incomplete_and_excess_provider_mappings() {
    let scope = SignatureBinderScopeV1::for_declaration(1, None);
    let uses = CanonicalBinderUseListV1::try_new(vec![binder(0, 0)]).unwrap();

    assert_eq!(
        uses.validate_semantics(2, &scope, &mut NoNominals),
        Err(BinderUseListSemanticValidationError::Arity {
            expected: 2,
            actual: 1,
        })
    );
    assert_eq!(
        uses.validate_semantics(0, &scope, &mut NoNominals),
        Err(BinderUseListSemanticValidationError::Arity {
            expected: 0,
            actual: 1,
        })
    );
}

#[test]
fn reports_the_out_of_scope_mapping_argument() {
    let uses = CanonicalBinderUseListV1::try_new(vec![binder(0, 0), binder(1, 0)]).unwrap();
    let scope = SignatureBinderScopeV1::for_declaration(1, None);

    assert_eq!(
        uses.validate_semantics(2, &scope, &mut NoNominals),
        Err(BinderUseListSemanticValidationError::Argument {
            index: 1,
            error: SignatureTypeSemanticError::BinderScope(
                SignatureBinderScopeError::DepthOutOfRange {
                    depth: 1,
                    available_depths: 1,
                }
            ),
        })
    );
}

#[test]
fn reports_the_mapping_argument_with_missing_nominal_authority() {
    let missing = nominal("Missing");
    let uses =
        CanonicalBinderUseListV1::try_new(vec![SignatureTypeKey::Nominal(missing.id())]).unwrap();
    let scope = SignatureBinderScopeV1::for_declaration(0, None);

    assert_eq!(
        uses.validate_semantics(1, &scope, &mut NoNominals),
        Err(BinderUseListSemanticValidationError::Argument {
            index: 0,
            error: SignatureTypeSemanticError::Reference(MissingNominal::Concrete(missing.id())),
        })
    );
}

#[test]
fn substitutes_both_provider_frames_in_flattened_owner_then_callable_order() {
    let owner_zero = SignatureTypeKey::Nominal(nominal("OwnerZero").id());
    let owner_one = SignatureTypeKey::Nominal(nominal("OwnerOne").id());
    let callable_zero = SignatureTypeKey::Nominal(nominal("CallableZero").id());
    let uses = CanonicalBinderUseListV1::try_new(vec![
        owner_zero.clone(),
        owner_one.clone(),
        callable_zero.clone(),
    ])
    .unwrap();
    let provider = DefaultTemplateProviderShapeV1::try_new(2, 1).unwrap();

    let cases = vec![
        (binder(1, 0), owner_zero.clone()),
        (binder(1, 1), owner_one.clone()),
        (binder(0, 0), callable_zero.clone()),
        (
            SignatureTypeKey::NominalApplication {
                origin: generic_nominal("Box"),
                arguments: NonEmptyVec::from_first(binder(1, 1), [binder(0, 0)]),
            },
            SignatureTypeKey::NominalApplication {
                origin: generic_nominal("Box"),
                arguments: NonEmptyVec::from_first(owner_one.clone(), [callable_zero.clone()]),
            },
        ),
        (
            SignatureTypeKey::Tuple(NonEmptyVec::from_first(binder(1, 0), [binder(0, 0)])),
            SignatureTypeKey::Tuple(NonEmptyVec::from_first(
                owner_zero.clone(),
                [callable_zero.clone()],
            )),
        ),
        (
            SignatureTypeKey::Function {
                effect: Effect::Suspend,
                parameters: vec![binder(1, 0)],
                result: Box::new(binder(0, 0)),
            },
            SignatureTypeKey::Function {
                effect: Effect::Suspend,
                parameters: vec![owner_zero.clone()],
                result: Box::new(callable_zero.clone()),
            },
        ),
        (
            SignatureTypeKey::RawPointer(Box::new(binder(1, 1))),
            SignatureTypeKey::RawPointer(Box::new(owner_one.clone())),
        ),
        (
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: vec![binder(1, 1)],
                result: Box::new(binder(0, 0)),
            },
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: vec![owner_one.clone()],
                result: Box::new(callable_zero),
            },
        ),
    ];

    for (source, expected) in cases {
        assert_eq!(
            uses.substitute_provider_type(provider, &source),
            Ok(expected)
        );
    }
}

#[test]
fn substitution_respects_compressed_single_provider_frames() {
    let first = SignatureTypeKey::Nominal(nominal("First").id());
    let second = SignatureTypeKey::Nominal(nominal("Second").id());
    let uses = CanonicalBinderUseListV1::try_new(vec![first, second.clone()]).unwrap();

    assert_eq!(
        uses.substitute_provider_type(
            DefaultTemplateProviderShapeV1::try_new(2, 0).unwrap(),
            &binder(0, 1),
        ),
        Ok(second.clone())
    );
    assert_eq!(
        uses.substitute_provider_type(
            DefaultTemplateProviderShapeV1::try_new(0, 2).unwrap(),
            &binder(0, 1),
        ),
        Ok(second)
    );
}

#[test]
fn substitution_rejects_mapping_arity_and_invalid_provider_binders() {
    let provider = DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap();
    let incomplete = CanonicalBinderUseListV1::try_new(vec![binder(0, 0)]).unwrap();
    assert_eq!(
        incomplete.substitute_provider_type(provider, &binder(0, 0)),
        Err(DefaultTemplateTypeSubstitutionError::MappingArity {
            expected: 2,
            actual: 1,
        })
    );

    let complete = CanonicalBinderUseListV1::try_new(vec![binder(0, 0), binder(0, 0)]).unwrap();
    assert_eq!(
        complete.substitute_provider_type(provider, &binder(2, 0)),
        Err(DefaultTemplateTypeSubstitutionError::ProviderBinder(
            SignatureBinderScopeError::DepthOutOfRange {
                depth: 2,
                available_depths: 2,
            },
        ))
    );
    assert_eq!(
        complete.substitute_provider_type(provider, &binder(0, 1)),
        Err(DefaultTemplateTypeSubstitutionError::ProviderBinder(
            SignatureBinderScopeError::IndexOutOfRange {
                depth: 0,
                index: 1,
                arity: 1,
            },
        ))
    );
}

const fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}

fn nominal(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap()
}

fn generic_nominal(name: &str) -> PersistentGenericTypeId {
    let key = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        1,
    );
    PersistentGenericTypeId::from_source_declaration(&key).unwrap()
}

struct NoNominals;

impl NominalInterfaceShapeAuthority<MissingNominal> for NoNominals {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, MissingNominal> {
        Err(MissingNominal::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, MissingNominal> {
        Err(MissingNominal::Generic(declaration))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MissingNominal {
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
}

impl std::fmt::Display for MissingNominal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing nominal {self:?}")
    }
}

impl std::error::Error for MissingNominal {}
