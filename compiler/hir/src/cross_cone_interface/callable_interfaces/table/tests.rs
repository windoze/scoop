use std::collections::BTreeMap;

use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, Effect, GcEffect, PackagePath,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{
    CallableDeclarationIdentityShapeV1, CallableImplementationV1, CallableInfixV1,
    CallableModalityV1, CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1,
    CanonicalBinderListV1, CanonicalSourceParameterShapesV1, NominalInterfaceShapeAuthority,
    PublicDeclarationOwnerV1, PublicLookupAccessV1, PublicNominalShapeV1, SourceParameterShapeV1,
    TypeParameterBinderV1, TypeParameterBoundsV1,
};

mod support_wire;

#[test]
fn producer_sorts_records_rejects_duplicates_and_has_stable_wire() {
    let first = fixture("First");
    let second = fixture("Second");
    let interfaces =
        CanonicalCallableInterfacesV1::try_new(vec![second.record.clone(), first.record.clone()])
            .unwrap();

    assert!(interfaces.records()[0].declaration() < interfaces.records()[1].declaration());
    assert_eq!(
        interfaces.get(first.record.declaration()),
        Some(&first.record)
    );
    assert!(!interfaces.is_empty());
    assert_eq!(
        CanonicalCallableInterfacesV1::try_new(vec![first.record.clone(), first.record.clone(),]),
        Err(CallableInterfaceSetBuildError::DuplicateDeclaration(
            first.record.declaration()
        ))
    );

    let expected = [
        b"\xa2\x01\x82".as_slice(),
        encode(&interfaces.records()[0]).unwrap().as_slice(),
        encode(&interfaces.records()[1]).unwrap().as_slice(),
        b"\x02\x80".as_slice(),
    ]
    .concat();
    assert_eq!(encode(&interfaces).unwrap(), expected);
}

#[test]
fn decoded_table_resolves_records_through_typed_authority() {
    let first = fixture("First");
    let second = fixture("Second");
    let expected =
        CanonicalCallableInterfacesV1::try_new(vec![first.record.clone(), second.record.clone()])
            .unwrap();
    let decoded = decode_table(&expected);
    let mut authority = identity_authority(&[&first, &second]);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_record_order() {
    let first = fixture("First");
    let second = fixture("Second");
    let (low, high) = if first.record.declaration() < second.record.declaration() {
        (&first, &second)
    } else {
        (&second, &first)
    };

    let duplicate = decode_table(&RecordSequence(vec![
        low.record.clone(),
        low.record.clone(),
    ]));
    let mut duplicate_authority = identity_authority(&[&first, &second]);
    assert!(matches!(
        duplicate.resolve(&mut duplicate_authority),
        Err(CallableInterfaceSetValidationError::DuplicateDeclaration { index: 1, .. })
    ));

    let reversed = decode_table(&RecordSequence(vec![
        high.record.clone(),
        low.record.clone(),
    ]));
    let mut reversed_authority = identity_authority(&[&first, &second]);
    assert_eq!(
        reversed
            .resolve(&mut reversed_authority)
            .unwrap_err()
            .to_string(),
        "non-canonical callable interface order at index 1"
    );
}

#[test]
fn table_semantics_reports_the_failing_canonical_record_index() {
    let first = fixture("First");
    let second = fixture("Second");
    let interfaces =
        CanonicalCallableInterfacesV1::try_new(vec![first.record.clone(), second.record.clone()])
            .unwrap();
    let mut authority = semantic_authority(&[&first, &second]);
    assert!(interfaces.validate_semantics(&mut authority).is_ok());

    let target = interfaces.records()[1].declaration();
    authority.shapes.insert(
        target,
        CallableDeclarationIdentityShapeV1::new(
            PublicDeclarationOwnerV1::TopLevel,
            1,
            0,
            None,
            Vec::new(),
        ),
    );
    assert!(matches!(
        interfaces.validate_semantics(&mut authority),
        Err(CallableInterfaceSetSemanticValidationError::Record {
            index: 1,
            error: CallableInterfaceSemanticValidationError::ParameterArity {
                expected: 0,
                actual: 1,
            },
        })
    ));
}

struct Fixture {
    identity: CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey>,
    record: CallableInterfaceRecordV1,
    identity_shape: CallableDeclarationIdentityShapeV1,
}

fn fixture(name: &str) -> Fixture {
    let key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        1,
        None,
        vec![binder()],
    );
    let identity = CborIdentityRecord::from_key(key).unwrap();
    let record = CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::GenericFunction(identity.id()),
        PublicDeclarationOwnerV1::TopLevel,
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            TypeParameterBoundsV1::Unconstrained,
        )])
        .unwrap(),
        None,
        CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
            CanonicalIdentifier::new("value").unwrap(),
            binder(),
        )])
        .unwrap(),
        binder(),
        CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            CallableSafetyV1::Safe,
            GcEffect::Managed,
            CallableImplementationV1::Scoop,
            CallableOperatorRoleV1::None,
            CallableInfixV1::Ordinary,
        )
        .unwrap(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
        crate::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap();
    let identity_shape = CallableDeclarationIdentityShapeV1::new(
        PublicDeclarationOwnerV1::TopLevel,
        1,
        0,
        None,
        vec![binder()],
    );
    Fixture {
        identity,
        record,
        identity_shape,
    }
}

fn identity_authority(fixtures: &[&Fixture]) -> ValidatedIdentityGraph {
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    for fixture in fixtures {
        pending
            .register_external_canonical_authority(fixture.identity.clone())
            .unwrap();
    }
    pending.finish().unwrap()
}

fn semantic_authority(fixtures: &[&Fixture]) -> SemanticAuthority {
    SemanticAuthority {
        shapes: fixtures
            .iter()
            .map(|fixture| (fixture.record.declaration(), fixture.identity_shape.clone()))
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SemanticAuthorityError {
    Callable(CallableDeclarationId),
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
}

impl std::fmt::Display for SemanticAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing semantic authority: {self:?}")
    }
}

impl std::error::Error for SemanticAuthorityError {}

struct SemanticAuthority {
    shapes: BTreeMap<CallableDeclarationId, CallableDeclarationIdentityShapeV1>,
}

impl NominalInterfaceShapeAuthority<SemanticAuthorityError> for SemanticAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, SemanticAuthorityError> {
        Err(SemanticAuthorityError::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, SemanticAuthorityError> {
        Err(SemanticAuthorityError::Generic(declaration))
    }
}

impl CallableInterfaceSemanticAuthority<SemanticAuthorityError> for SemanticAuthority {
    fn callable_declaration_identity_shape(
        &mut self,
        declaration: CallableDeclarationId,
    ) -> Result<CallableDeclarationIdentityShapeV1, SemanticAuthorityError> {
        self.shapes
            .get(&declaration)
            .cloned()
            .ok_or(SemanticAuthorityError::Callable(declaration))
    }
}

fn decode_table<T: WireEncode>(value: &T) -> DecodedCanonicalCallableInterfacesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

const fn binder() -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index: 0 }
}

struct RecordSequence(Vec<CallableInterfaceRecordV1>);

impl WireEncode for RecordSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(0)
    }
}
