//! Shared callable ABI payload and provider checks.

use super::*;
use crate::{
    CallableAbiBuildError, CallableAbiDecodeError, CallableAbiRecordV1, CallableAbiValidationError,
    DecodedCallableAbiRecordV1, StrongObjectSymbolSurfaceV1,
};
use scoop_wire::{decode_canonical, encode};

#[test]
fn shared_abi_decodes_with_the_actual_provider_and_rejects_another_provider() {
    for provider in [Fixture::new("provider").producer, ConeIdentity::CORE] {
        let fixture = Fixture::for_producer(provider, "exported");
        let record = fixture.export().callable_abi().clone();
        let bytes = encode(&record).unwrap();
        assert_eq!(bytes[0], 0xa6);
        assert_eq!(
            decode_record(&bytes)
                .validate(provider, &mut fixture.identities(&[]))
                .unwrap(),
            record
        );
        assert!(matches!(
            decode_record(&bytes).validate(ConeIdentity::SINGLE_FILE, &mut fixture.identities(&[])),
            Err(CallableAbiDecodeError::RecordMismatch)
        ));
        let definitions =
            StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&fixture.foundation).unwrap();
        record
            .validate_against(&fixture.foundation, &definitions)
            .unwrap();
        assert!(matches!(
            record.validate_definition(ConeIdentity::SINGLE_FILE, &definitions),
            Err(CallableAbiValidationError::ContractMismatch)
        ));
    }
}

#[test]
fn shared_abi_rejects_changed_symbol_and_definition_without_resolving_derived_ids() {
    let fixture = Fixture::new("changedLinkContract");
    let export = fixture.export();
    for needle in [
        export.expected_symbol().key().owner_bytes().to_vec(),
        export.required_definition().as_array().to_vec(),
    ] {
        let mut bytes = encode(export.callable_abi()).unwrap();
        let positions = bytes
            .windows(needle.len())
            .enumerate()
            .filter_map(|(index, value)| (value == needle).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        bytes[positions[0] + needle.len() - 1] ^= 1;
        assert!(matches!(
            decode_record(&bytes).validate(fixture.producer, &mut fixture.identities(&[])),
            Err(CallableAbiDecodeError::RecordMismatch)
        ));
    }
}

#[test]
fn shared_abi_rejects_gc_protocol_mismatch_on_the_read_path() {
    let fixture = Fixture::new("changedRootPlan");
    let export = fixture.export();
    let mut bytes = encode(export.callable_abi()).unwrap();
    let root_offset = bytes.len() - encode(&export.required_definition()).unwrap().len() - 2;
    assert_eq!(&bytes[root_offset - 1..=root_offset + 1], &[5, 2, 6]);
    bytes[root_offset] = 1;
    assert!(matches!(
        decode_record(&bytes).validate(fixture.producer, &mut fixture.identities(&[])),
        Err(CallableAbiDecodeError::Build(
            CallableAbiBuildError::RootProtocolMismatch {
                abi: GcEffect::NoGc,
                root: GcEffect::Managed,
            }
        ))
    ));
}

#[test]
fn shared_abi_rejects_suspend_and_declaration_target_mismatch() {
    let fixture = Fixture::new("suspend");
    let signature = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(Effect::Suspend, None, Vec::new(), fixture.unit),
        Vec::new(),
        ScoopAbiReturn::unit_void(),
        GcEffect::Managed,
    )
    .unwrap();
    assert!(matches!(
        CallableAbiRecordV1::new(
            fixture.producer,
            fixture.target,
            signature,
            CallingConvention::Cdecl,
            ExternalCallableRootPlan::ManagedStatepoint
        ),
        Err(CallableAbiBuildError::Suspend)
    ));
    let other = Fixture::new("anotherDeclaration");
    assert!(matches!(
        ParamFreeLirCallableExportV1::from_abi(other.declaration, fixture.export().callable_abi().clone()),
        Err(ParamFreeLirCallableBuildError::TargetMismatch { declaration, expected, actual })
            if declaration == other.declaration && expected == other.target && actual == fixture.target
    ));
}

fn decode_record(bytes: &[u8]) -> DecodedCallableAbiRecordV1 {
    decode_canonical(bytes).unwrap()
}
