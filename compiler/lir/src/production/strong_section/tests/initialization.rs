//! Field replay owns the ABI while leaving all other Strong checks pending.

use scoop_identity::*;
use scoop_wire::{BudgetMeter, WireEncode};

use super::*;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn decode<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}

pub(super) fn check(section: &StrongProductionSectionV2) {
    let abi = crate::initialization_cycle_abi_for_test();
    for expected in [None, Some(Box::new(abi.clone()))] {
        let mut raw: DecodedStrongProductionSectionV2 = decode(section);
        raw.initialization_cycle_abi = expected
            .as_ref()
            .map(|value| Box::new(decode(value.as_ref())));
        let original = encode(&raw).unwrap();
        let validated = raw
            .validate_initialization_abi(expected.clone(), &mut meter())
            .unwrap();
        assert_eq!(validated.initialization_cycle_abi(), expected.as_deref());
        assert_eq!(encode(&validated).unwrap(), original);
    }
    let mut extra: DecodedStrongProductionSectionV2 = decode(section);
    extra.initialization_cycle_abi = Some(Box::new(decode(&abi)));
    assert!(matches!(
        extra.validate_initialization_abi(None, &mut meter()),
        Err(StrongInitializationAbiValidationError::Mismatch)
    ));
    let missing: DecodedStrongProductionSectionV2 = decode(section);
    assert!(matches!(
        missing.validate_initialization_abi(Some(Box::new(abi.clone())), &mut meter()),
        Err(StrongInitializationAbiValidationError::Mismatch)
    ));
    let other = donor(&abi);
    let storage = CanonicalScoopStorage::new(
        abi.abi_signature().signature().parameters()[0],
        4,
        std::num::NonZeroU64::new(4).unwrap(),
        ScoopAbiValueShape::Scalar,
    );
    let signature = CanonicalScoopAbiFunctionSignature::new(
        abi.abi_signature().signature().clone(),
        vec![ScoopAbiArgument::direct(storage).unwrap()],
        ScoopAbiReturn::UnitVoid,
        GcEffect::Managed,
    )
    .unwrap();
    let narrowed = CallableAbiRecordV1::new(
        ConeIdentity::CORE,
        abi.target(),
        signature,
        crate::CallingConvention::Cdecl,
        crate::ExternalCallableRootPlan::ManagedStatepoint,
    )
    .unwrap();
    let mut raw: DecodedStrongProductionSectionV2 = decode(section);
    raw.initialization_cycle_abi = Some(Box::new(decode(&narrowed)));
    assert!(matches!(
        raw.validate_initialization_abi(Some(Box::new(abi.clone())), &mut meter()),
        Err(StrongInitializationAbiValidationError::Mismatch)
    ));
    for field in [1, 2, 3, 5, 6] {
        let mut raw: DecodedStrongProductionSectionV2 = decode(section);
        raw.initialization_cycle_abi = Some(Box::new(decode(&Changed {
            original: &abi,
            other: &other,
            field,
        })));
        assert!(
            matches!(
                raw.validate_initialization_abi(Some(Box::new(abi.clone())), &mut meter()),
                Err(StrongInitializationAbiValidationError::Mismatch)
            ),
            "field {field}"
        );
    }
    assert!(
        decode_canonical::<DecodedCallableAbiRecordV1>(
            &encode(&Changed {
                original: &abi,
                other: &other,
                field: 4
            })
            .unwrap(),
            DecodeLimits::default()
        )
        .is_err()
    );
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        let raw: DecodedStrongProductionSectionV2 = decode(section);
        assert!(matches!(
            raw.validate_initialization_abi(None, &mut BudgetMeter::new(limits)),
            Err(StrongInitializationAbiValidationError::Resource(_))
        ));
    }
}

fn donor(abi: &CallableAbiRecordV1) -> CallableAbiRecordV1 {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let target = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("another").unwrap(),
        0,
        None,
        vec![],
    ))
    .unwrap();
    let original = abi.abi_signature();
    let signature = CanonicalScoopAbiFunctionSignature::new(
        original.signature().clone(),
        original.arguments().to_vec(),
        original.result(),
        GcEffect::NoGc,
    )
    .unwrap();
    CallableAbiRecordV1::new(
        ConeIdentity::SINGLE_FILE,
        StrongCallableDefinitionOwner::Function(target),
        signature,
        crate::CallingConvention::Cdecl,
        crate::ExternalCallableRootPlan::NoGc,
    )
    .unwrap()
}

struct Changed<'a> {
    original: &'a CallableAbiRecordV1,
    other: &'a CallableAbiRecordV1,
    field: u32,
}
impl WireEncode for Changed<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        for field in 1..=6 {
            encoder.field(field)?;
            let record = if field == self.field {
                self.other
            } else {
                self.original
            };
            match field {
                1 => record.target().encode(encoder)?,
                2 => record.abi_signature().encode(encoder)?,
                3 => record.expected_symbol().encode(encoder)?,
                4 if self.field == 4 => encoder.unsigned(99)?,
                4 => record.calling_convention().encode(encoder)?,
                5 => record.root_plan().encode(encoder)?,
                6 => record.required_definition().encode(encoder)?,
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}
