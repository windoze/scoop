use scoop_identity::{
    CallableMaterializationContext, CallbackApplicationKey, CallbackMode, CallbackParameterIndex,
    CallbackRegistrationKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    CoreBuiltinNominal, DeclarationScope, DecodedCallbackRegistrationKey,
    DecodedCborIdentityRecord, DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey,
    GeneratedCallableKey, IdentityLayer, LexicalCallableParent, PackagePath,
    PendingIdentityValidation, PersistentCallbackRegistrationId, PersistentFunctionId,
    PersistentTypeId, SignatureCallableShape, SignatureTypeKey, SourceCAbiFunctionSignature,
    SourceCAbiReturn, SourceDeclarationKey, SourceDeclarationSite, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;
use crate::ForeignCallbackStorageAbi;

type DecodedRegistrationRecord =
    DecodedCborIdentityRecord<PersistentCallbackRegistrationId, DecodedCallbackRegistrationKey>;

struct CallbackFixture {
    canonical: CanonicalMirFoundation,
    registration: DecodedRegistrationRecord,
    function: PersistentFunctionId,
    nominal: PersistentTypeId,
    application: PersistentCallbackApplicationId,
}

fn callback_fixture(include_record: bool, mode: CallbackMode) -> CallbackFixture {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let function_key = SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("invoke").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function = PersistentFunctionId::from_source_declaration(&function_key).unwrap();
    let nominal = CoreBuiltinNominal::Unit.identity_record().id();
    let registration_key = CallbackRegistrationKey::new(
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
    let registration_record: CborIdentityRecord<
        PersistentCallbackRegistrationId,
        CallbackRegistrationKey,
    > = CborIdentityRecord::from_key(registration_key.clone()).unwrap();
    let registration = decode_canonical(
        &encode(&registration_record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let application_key = CallbackApplicationKey::new(
        &registration_key,
        CallableMaterializationContext::NoSubstitution,
    )
    .unwrap();
    let application_record = CborIdentityRecord::from_key(application_key).unwrap();
    let application = application_record.id();
    let exact_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();
    let exact = exact_record.id();
    let adapter_record =
        CborIdentityRecord::from_key(GeneratedCallableKey::ForeignCallbackManagedAdapter {
            application,
        })
        .unwrap();
    let subject = CallableSignatureSubject::strong(CallableOwner::Generated(adapter_record.id()));
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact);

    let mut canonical = CanonicalMirFoundation::empty();
    canonical.set_exact_types(vec![exact_record]).unwrap();
    canonical
        .set_generated_callables(vec![adapter_record])
        .unwrap();
    canonical
        .set_callable_signatures(vec![CallableSignatureRecord::new(
            subject,
            signature.clone(),
        )])
        .unwrap();
    canonical
        .set_callback_applications(vec![application_record])
        .unwrap();
    if include_record {
        canonical
            .set_callback_application_records(vec![CallbackApplicationRecord::new(
                application,
                subject,
                signature,
                ForeignCallbackStorageAbi::ClosureResultRootsThrowableToU32,
                mode,
            )])
            .unwrap();
    }
    CallbackFixture {
        canonical,
        registration,
        function,
        nominal,
        application,
    }
}

fn decode(canonical: &CanonicalMirFoundation) -> DecodedMirFoundation {
    decode_canonical(&encode(canonical).unwrap(), DecodeLimits::default()).unwrap()
}

fn validate_identities(
    decoded: &DecodedMirFoundation,
    fixture: &CallbackFixture,
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(fixture.function).unwrap();
    pending.register_authority(fixture.nominal).unwrap();
    pending
        .register(IdentityLayer::Hir, &fixture.registration)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    pending.resolve(&fixture.registration).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}

#[test]
fn validates_callback_identity_record_and_signature_as_one_relation() {
    let fixture = callback_fixture(true, CallbackMode::Reusable);
    let bytes = encode(&fixture.canonical).unwrap();
    let decoded = decode(&fixture.canonical);
    let mut identities = validate_identities(&decoded, &fixture);

    let validated = decoded.validate(&mut identities).unwrap();

    assert_eq!(encode(&validated).unwrap(), bytes);
    assert_eq!(validated.counts().callback_applications, 1);
    assert_eq!(validated.counts().callback_application_records, 1);
}

#[test]
fn rejects_a_callback_identity_without_its_semantic_record() {
    let fixture = callback_fixture(false, CallbackMode::Reusable);
    let decoded = decode(&fixture.canonical);
    let mut identities = validate_identities(&decoded, &fixture);

    assert!(matches!(
        decoded.validate(&mut identities),
        Err(MirFoundationValidationError::CallbackRelation(
            CallbackApplicationRelationError::MissingRecord { application }
        )) if application == fixture.application
    ));
}

#[test]
fn rejects_a_callback_record_that_changes_the_registration_mode() {
    let fixture = callback_fixture(true, CallbackMode::OneShot);
    let decoded = decode(&fixture.canonical);
    let mut identities = validate_identities(&decoded, &fixture);

    assert!(matches!(
        decoded.validate(&mut identities),
        Err(MirFoundationValidationError::CallbackRelation(
            CallbackApplicationRelationError::ModeMismatch { application }
        )) if application == fixture.application
    ));
}

#[test]
fn rejects_semantically_noncanonical_identity_order() {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let any = CoreBuiltinNominal::Any.identity_record().id();
    let mut canonical = CanonicalMirFoundation::empty();
    canonical
        .set_exact_types(vec![
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit)).unwrap(),
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(any)).unwrap(),
        ])
        .unwrap();
    let mut decoded = decode(&canonical);
    decoded.decoded.exact_types.swap(0, 1);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(unit).unwrap();
    pending.register_authority(any).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let mut identities = pending.finish().unwrap();

    assert!(matches!(
        decoded.validate(&mut identities),
        Err(MirFoundationValidationError::NonCanonicalFoundation)
    ));
}
