use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    PackagePath, PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

use super::*;
use crate::{PublicNominalShapeV1, SignatureBinderScopeError};

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
