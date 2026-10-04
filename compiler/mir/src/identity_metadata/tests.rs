use std::fmt;

use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableMaterializationContext,
    CallableOdrMemberId, CallableOwner, CallbackApplicationKey, CallbackMode,
    CallbackParameterIndex, CallbackRegistrationKey, CanonicalIdentifier, ConeIdentity,
    DeclarationScope, DecodedPersistentId, DefinitionOwnerChain, Effect, ExactCallableSignature,
    ExactTypeKey, LexicalCallableParent, OdrGroupId, OdrMemberDiscriminator, OdrMemberId,
    OdrMemberKey, OdrMemberRole, PackagePath, PersistentCallableApplicationId,
    PersistentCallbackApplicationId, PersistentConstructorId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentIdMismatch, PersistentIdResolver, PersistentPropertyAccessorId, PersistentTypeId,
    SignatureCallableShape, SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind, SpecializationKey,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{decode_canonical, encode};

use super::{
    CallableSignatureRecord, CallableSignatureResolver, CallableSignatureSubject,
    CallbackApplicationRecord, DecodedCallableSignatureRecord, DecodedCallbackApplicationRecord,
    ForeignCallbackStorageAbi,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("test reference does not exist")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver {
    function: PersistentFunctionId,
    application: PersistentCallableApplicationId,
    callback: PersistentCallbackApplicationId,
    exact: PersistentExactTypeId,
    callable_member: Option<CallableOdrMemberId>,
}

macro_rules! reject_resolution {
    ($id:ty) => {
        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, _decoded: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                Err(ResolutionError)
            }
        }
    };
}

reject_resolution!(PersistentGenericFunctionId);
reject_resolution!(PersistentConstructorId);
reject_resolution!(PersistentPropertyAccessorId);
reject_resolution!(PersistentGeneratedCallableId);

impl PersistentIdResolver<PersistentFunctionId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<PersistentFunctionId, Self::Error> {
        verify(decoded, self.function)
    }
}

impl PersistentIdResolver<PersistentCallableApplicationId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentCallableApplicationId>,
    ) -> Result<PersistentCallableApplicationId, Self::Error> {
        verify(decoded, self.application)
    }
}

impl PersistentIdResolver<PersistentCallbackApplicationId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentCallbackApplicationId>,
    ) -> Result<PersistentCallbackApplicationId, Self::Error> {
        verify(decoded, self.callback)
    }
}

impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        verify(decoded, self.exact)
    }
}

impl CallableSignatureResolver<ResolutionError> for Resolver {
    fn resolve_callable_odr_member(
        &mut self,
        decoded: DecodedPersistentId<OdrMemberId>,
    ) -> Result<CallableOdrMemberId, ResolutionError> {
        let member = self.callable_member.ok_or(ResolutionError)?;
        verify(decoded, member.member()).map(|_| member)
    }
}

#[test]
fn callable_signature_records_roundtrip_strong_and_odr_subjects() {
    let fixture = Fixture::new();
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, vec![fixture.exact], fixture.exact);
    let strong = CallableSignatureRecord::new(
        CallableSignatureSubject::strong(CallableOwner::Function(fixture.function)),
        signature.clone(),
    );
    let odr = CallableSignatureRecord::new(
        CallableSignatureSubject::odr(fixture.callable_member),
        signature,
    );

    for record in [strong, odr] {
        let bytes = encode(&record).unwrap();
        let decoded = decode_canonical::<DecodedCallableSignatureRecord>(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), record);
    }
}

#[test]
fn callback_application_record_roundtrips_all_fields() {
    let fixture = Fixture::new();
    let record = CallbackApplicationRecord::new(
        fixture.callback,
        CallableSignatureSubject::strong(CallableOwner::Function(fixture.function)),
        ExactCallableSignature::new(
            Effect::Suspend,
            Some(fixture.exact),
            vec![fixture.exact],
            fixture.exact,
        ),
        ForeignCallbackStorageAbi::ClosureContextResultRootsThrowableToStatus,
        CallbackMode::OneShot,
    );
    let bytes = encode(&record).unwrap();
    let decoded = decode_canonical::<DecodedCallbackApplicationRecord>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), record);
}

#[test]
fn subject_order_uses_subject_tag_then_raw_identity() {
    let fixture = Fixture::new();
    let strong = CallableSignatureSubject::strong(CallableOwner::Function(fixture.function));
    let odr = CallableSignatureSubject::odr(fixture.callable_member);
    assert!(strong.compare_sort_key(odr).is_lt());

    let other_strong =
        CallableSignatureSubject::strong(CallableOwner::Application(fixture.application));
    assert_eq!(
        strong.compare_sort_key(other_strong),
        strong.raw_id().cmp(&other_strong.raw_id())
    );
}

#[test]
fn unknown_subject_and_storage_tags_are_rejected() {
    let fixture = Fixture::new();
    let record = CallableSignatureRecord::new(
        CallableSignatureSubject::strong(CallableOwner::Function(fixture.function)),
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), fixture.exact),
    );
    let mut bytes = encode(&record).unwrap();
    let subject_tag = bytes
        .windows(3)
        .position(|window| window == [0xa2, 0x00, 0x01])
        .unwrap()
        + 2;
    bytes[subject_tag] = 3;
    assert!(decode_canonical::<DecodedCallableSignatureRecord>(&bytes).is_err());
    assert!(decode_canonical::<ForeignCallbackStorageAbi>(&[1]).is_err());
    assert!(decode_canonical::<ForeignCallbackStorageAbi>(&[3]).is_err());
}

#[test]
fn odr_subject_must_resolve_through_callable_refinement() {
    let fixture = Fixture::new();
    let record = CallableSignatureRecord::new(
        CallableSignatureSubject::odr(fixture.callable_member),
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), fixture.exact),
    );
    let decoded =
        decode_canonical::<DecodedCallableSignatureRecord>(&encode(&record).unwrap()).unwrap();
    let mut resolver = fixture.resolver();
    resolver.callable_member = None;
    assert!(decoded.resolve(&mut resolver).is_err());
}

struct Fixture {
    function: PersistentFunctionId,
    application: PersistentCallableApplicationId,
    callback: PersistentCallbackApplicationId,
    exact: PersistentExactTypeId,
    callable_member: CallableOdrMemberId,
}

impl Fixture {
    fn new() -> Self {
        let site = || {
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap()
        };
        let nominal = SourceDeclarationKey::nominal(
            site(),
            CanonicalIdentifier::new("Value").unwrap(),
            SourceNominalKind::Class,
            0,
        );
        let nominal = PersistentTypeId::from_source_declaration(&nominal).unwrap();
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
        let function_key = SourceDeclarationKey::function(
            site(),
            CanonicalIdentifier::new("invoke").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
        let application_key =
            CallableApplicationKey::for_function(function, CallableInstantiationOwner::NoOwner);
        let application = PersistentCallableApplicationId::from_key(&application_key).unwrap();
        let group = OdrGroupId::from_key(&SpecializationKey::Callable {
            application: application_key,
        })
        .unwrap();
        let callable_member_key = OdrMemberKey::new(
            group,
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::CallableApplication(application),
        )
        .unwrap();
        let callable_member = CallableOdrMemberId::from_key(&callable_member_key).unwrap();

        let registration = CallbackRegistrationKey::new(
            LexicalCallableParent::function(function),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
                [],
            ),
            SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
            CallbackParameterIndex::new(0),
            SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                SignatureTypeKey::Nominal(nominal),
            ),
            CallbackMode::Reusable,
        );
        let callback_key = CallbackApplicationKey::new(
            &registration,
            CallableMaterializationContext::NoSubstitution,
        )
        .unwrap();
        let callback = PersistentCallbackApplicationId::from_key(&callback_key).unwrap();
        Self {
            function,
            application,
            callback,
            exact,
            callable_member,
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            function: self.function,
            application: self.application,
            callback: self.callback,
            exact: self.exact,
            callable_member: Some(self.callable_member),
        }
    }
}

fn verify<I: scoop_identity::PersistentId>(
    decoded: DecodedPersistentId<I>,
    expected: I,
) -> Result<I, ResolutionError> {
    decoded
        .verify(expected)
        .map_err(|_: PersistentIdMismatch<I>| ResolutionError)
}
