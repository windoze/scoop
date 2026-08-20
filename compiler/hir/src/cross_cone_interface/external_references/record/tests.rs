use scoop_wire::{WireEncode, decode_canonical, encode};

use super::*;
use crate::ExternalHirReferenceRoleV1;
use crate::cross_cone_interface::external_references::test_support::{Fixture, roles, witnesses};

#[test]
fn record_has_a_fixed_six_field_wire_and_round_trips() {
    let fixture = Fixture::new();
    let record = fixture.alias_reference(fixture.first_alias, fixture.first_route.clone());
    let expected = [
        b"\xa6\x01".as_slice(),
        encode(&record.origin()).unwrap().as_slice(),
        b"\x02".as_slice(),
        encode(&record.target()).unwrap().as_slice(),
        b"\x03".as_slice(),
        encode(record.roles()).unwrap().as_slice(),
        b"\x04".as_slice(),
        encode(record.witnesses()).unwrap().as_slice(),
        b"\x05\x80\x06\x80".as_slice(),
    ]
    .concat();

    assert_eq!(encode(&record).unwrap(), expected);
    assert_eq!(record.origin(), fixture.provider);
    assert_eq!(
        record.target(),
        ExternalHirTargetV1::TypeAlias(fixture.first_alias)
    );

    let decoded: DecodedExternalHirReferenceV1 = decode_canonical(&expected).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.authority()).unwrap(), record);
}

#[test]
fn source_name_roles_require_witnesses() {
    let fixture = Fixture::new();
    let empty = CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap();

    assert_eq!(
        ExternalHirReferenceV1::try_new(
            fixture.provider,
            ExternalHirTargetV1::TypeAlias(fixture.first_alias),
            roles(&[ExternalHirReferenceRoleV1::ReexportTarget]),
            empty,
            Default::default(),
            Default::default(),
        ),
        Err(ExternalHirReferenceBuildError::MissingWitness)
    );
}

#[test]
fn non_source_name_roles_reject_witnesses_and_mixed_roles_accept_them() {
    let fixture = Fixture::new();
    let route_witnesses = || witnesses(vec![fixture.first_route.clone()]);

    assert_eq!(
        ExternalHirReferenceV1::try_new(
            fixture.provider,
            ExternalHirTargetV1::TypeAlias(fixture.first_alias),
            roles(&[ExternalHirReferenceRoleV1::SignatureDependency]),
            route_witnesses(),
            Default::default(),
            Default::default(),
        ),
        Err(ExternalHirReferenceBuildError::UnexpectedWitness)
    );
    assert!(
        ExternalHirReferenceV1::try_new(
            fixture.provider,
            ExternalHirTargetV1::TypeAlias(fixture.first_alias),
            roles(&[
                ExternalHirReferenceRoleV1::SignatureDependency,
                ExternalHirReferenceRoleV1::DefaultDependency,
            ]),
            route_witnesses(),
            Default::default(),
            Default::default(),
        )
        .is_ok()
    );
}

#[test]
fn decoded_record_rechecks_role_witness_shape() {
    let fixture = Fixture::new();
    let malformed = RawReference {
        origin: fixture.provider,
        target: ExternalHirTargetV1::TypeAlias(fixture.first_alias),
        roles: roles(&[ExternalHirReferenceRoleV1::ReexportTarget]),
        witnesses: CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
    };
    let decoded: DecodedExternalHirReferenceV1 =
        decode_canonical(&encode(&malformed).unwrap()).unwrap();

    assert!(matches!(
        decoded.resolve(&mut fixture.authority()),
        Err(ExternalHirReferenceResolutionError::Shape(
            ExternalHirReferenceBuildError::MissingWitness
        ))
    ));
}

struct RawReference {
    origin: scoop_identity::ConeIdentity,
    target: ExternalHirTargetV1,
    roles: CanonicalExternalHirReferenceRolesV1,
    witnesses: CanonicalDependencyBindingWitnessesV1,
}

impl WireEncode for RawReference {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.roles.encode(encoder)?;
        encoder.field(4)?;
        self.witnesses.encode(encoder)?;
        encoder.field(5)?;
        encoder.array(0)?;
        encoder.field(6)?;
        encoder.array(0)
    }
}

#[test]
fn aliases_and_defaults_round_trip_as_resolved_declarations() {
    let fixture = Fixture::new();
    for role in [
        ExternalHirReferenceRoleV1::AliasTarget,
        ExternalHirReferenceRoleV1::DefaultDependency,
    ] {
        let record = ExternalHirReferenceV1::try_new(
            fixture.provider,
            ExternalHirTargetV1::TypeAlias(fixture.first_alias),
            roles(&[role]),
            witnesses(Vec::new()),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let decoded: DecodedExternalHirReferenceV1 =
            decode_canonical(&encode(&record).unwrap()).unwrap();
        assert_eq!(decoded.resolve(&mut fixture.authority()).unwrap(), record);
    }
}
