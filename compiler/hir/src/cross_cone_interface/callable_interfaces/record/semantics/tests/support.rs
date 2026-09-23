use std::collections::BTreeMap;

use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope,
    DefinitionOwnerAtom, DefinitionOwnerChain, Effect, GcEffect, NominalDeclarationOwner,
    NonEmptyVec, PackagePath, PersistentExtensionPropertyId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

use super::super::*;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1,
    CanonicalSignatureTypesV1, CanonicalSourceParameterShapesV1, NominalInterfaceShapeAuthority,
    NominalTypeParameterBoundsV1, PublicLookupAccessV1, PublicNominalKindV1, PublicNominalShapeV1,
    SourceParameterShapeV1, TypeParameterBinderV1, TypeParameterBoundsV1,
};

pub(super) struct Fixture {
    pub(super) declaration: CallableTemplateOrigin,
    pub(super) owner: PublicDeclarationOwnerV1,
    pub(super) contract: PersistentGenericTypeId,
    pub(super) result: PersistentTypeId,
    identity: CallableDeclarationIdentityShapeV1,
    generic_nominals: BTreeMap<PersistentGenericTypeId, PublicNominalShapeV1>,
    concrete_nominals: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let owner_key = nominal("Owner", SourceNominalKind::Class, 1);
        let owner_id = PersistentGenericTypeId::from_source_declaration(&owner_key).unwrap();
        let owner =
            PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::GenericTemplate(owner_id));
        let contract_key = nominal("Contract", SourceNominalKind::Interface, 1);
        let contract = PersistentGenericTypeId::from_source_declaration(&contract_key).unwrap();
        let result_key = nominal("Result", SourceNominalKind::Struct, 0);
        let result = PersistentTypeId::from_source_declaration(&result_key).unwrap();
        let parameter_types = vec![outer_binder(0), own_binder(0)];
        let function_key = SourceDeclarationKey::function(
            owned_site(DefinitionOwnerAtom::GenericType(owner_id)),
            identifier("combine"),
            1,
            None,
            parameter_types.clone(),
        );
        let declaration = CallableTemplateOrigin::GenericFunction(
            PersistentGenericFunctionId::from_source_declaration(&function_key).unwrap(),
        );
        Self {
            declaration,
            owner,
            contract,
            result,
            identity: CallableDeclarationIdentityShapeV1::new(owner, 1, 1, None, parameter_types),
            generic_nominals: BTreeMap::from([
                (
                    owner_id,
                    PublicNominalShapeV1::new(PublicNominalKindV1::Class, 1),
                ),
                (
                    contract,
                    PublicNominalShapeV1::new(PublicNominalKindV1::Interface, 1),
                ),
            ]),
            concrete_nominals: BTreeMap::from([(
                result,
                PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0),
            )]),
        }
    }

    pub(super) fn record(&self) -> CallableInterfaceRecordV1 {
        self.record_with(
            binders_with_bound(self.contract, outer_binder(0)),
            vec![outer_binder(0), own_binder(0)],
            SignatureTypeKey::Nominal(self.result),
        )
    }

    pub(super) fn record_with(
        &self,
        type_parameters: CanonicalBinderListV1,
        parameter_types: Vec<SignatureTypeKey>,
        result: SignatureTypeKey,
    ) -> CallableInterfaceRecordV1 {
        CallableInterfaceRecordV1::try_new(
            self.declaration,
            self.owner,
            type_parameters,
            None,
            parameters(parameter_types),
            result,
            scoop_effects(),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
            crate::CanonicalPersistentIdsV1::empty(),
        )
        .unwrap()
    }

    pub(super) fn authority(&self) -> TestAuthority {
        self.authority_for(self.declaration, self.identity.clone())
    }

    pub(super) fn authority_for(
        &self,
        declaration: CallableTemplateOrigin,
        identity: CallableDeclarationIdentityShapeV1,
    ) -> TestAuthority {
        TestAuthority {
            declaration,
            identity,
            generic_nominals: self.generic_nominals.clone(),
            concrete_nominals: self.concrete_nominals.clone(),
        }
    }

    pub(super) fn extension_property(&self) -> PersistentExtensionPropertyId {
        let key = SourceDeclarationKey::extension_property(
            top_level_site(),
            identifier("content"),
            1,
            own_binder(0),
        );
        PersistentExtensionPropertyId::from_source_declaration(&key).unwrap()
    }

    pub(super) fn missing_result(&self) -> PersistentTypeId {
        PersistentTypeId::from_source_declaration(&nominal("Missing", SourceNominalKind::Struct, 0))
            .unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestAuthorityError {
    Callable(CallableTemplateOrigin),
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test authority: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

pub(super) struct TestAuthority {
    pub(super) declaration: CallableTemplateOrigin,
    pub(super) identity: CallableDeclarationIdentityShapeV1,
    generic_nominals: BTreeMap<PersistentGenericTypeId, PublicNominalShapeV1>,
    concrete_nominals: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for TestAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.concrete_nominals
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.generic_nominals
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Generic(declaration))
    }
}

impl CallableInterfaceSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn callable_declaration_identity_shape(
        &mut self,
        declaration: CallableTemplateOrigin,
    ) -> Result<CallableDeclarationIdentityShapeV1, TestAuthorityError> {
        if declaration == self.declaration {
            Ok(self.identity.clone())
        } else {
            Err(TestAuthorityError::Callable(declaration))
        }
    }
}

pub(super) fn binders_with_bound(
    contract: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> CanonicalBinderListV1 {
    let bounds = NominalTypeParameterBoundsV1::try_new(
        None,
        CanonicalSignatureTypesV1::try_new(vec![SignatureTypeKey::NominalApplication {
            origin: contract,
            arguments: NonEmptyVec::new(vec![argument]).unwrap(),
        }])
        .unwrap(),
    )
    .unwrap();
    CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        identifier("U"),
        TypeParameterBoundsV1::Nominal(bounds),
    )])
    .unwrap()
}

pub(super) fn empty_binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(Vec::new()).unwrap()
}

pub(super) fn parameters(types: Vec<SignatureTypeKey>) -> CanonicalSourceParameterShapesV1 {
    CanonicalSourceParameterShapesV1::try_new(
        types
            .into_iter()
            .enumerate()
            .map(|(index, value_type)| {
                SourceParameterShapeV1::new(identifier(&format!("p{index}")), value_type)
            })
            .collect(),
    )
    .unwrap()
}

pub(super) fn scoop_effects() -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Safe,
        GcEffect::Managed,
        CallableImplementationV1::Scoop,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

pub(super) const fn own_binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

pub(super) const fn outer_binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 1, index }
}

pub(super) const fn deep_binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 2, index }
}

fn nominal(name: &str, kind: SourceNominalKind, arity: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(top_level_site(), identifier(name), kind, arity)
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
