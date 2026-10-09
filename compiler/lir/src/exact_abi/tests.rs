use scoop_identity::*;
use scoop_wire::{WireEncode, decode_canonical, encode};

use super::*;
use crate::exact_layout::tests::{Bound, exact, field, integer, managed, source, unit};
use crate::*;

mod fixtures;
mod reader;
mod table;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

#[test]
fn abi_records_preserve_receiver_order_and_carrier_plans() {
    let unit: ExactLayoutExportV1 = unit().into();
    let byte: ExactLayoutExportV1 = integer("Byte", IntegerKind::SIGNED_8).into();
    let reference: ExactLayoutExportV1 = managed().into();
    let aggregate = fixtures::aggregate(false);
    let zst = fixtures::aggregate(true);
    let parameters = [&unit, &byte, &aggregate, &zst, &byte];
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(reference.identity().exact()),
        parameters
            .iter()
            .map(|layout| layout.identity().exact())
            .collect(),
        aggregate.identity().exact(),
    );
    let (target, foundation) = fixtures::foundation("invoke", true);
    let value = ExactCallableAbiExportV1::from_signature(
        TARGET,
        target,
        scoop_identity::CanonicalScoopAbiFunctionSignature::new(
            signature,
            vec![
                ScoopAbiArgument::direct(reference.value_handle().unwrap().canonical_storage())
                    .unwrap(),
                ScoopAbiArgument::elided_zst(unit.value_handle().unwrap().canonical_storage())
                    .unwrap(),
                ScoopAbiArgument::direct(byte.value_handle().unwrap().canonical_storage()).unwrap(),
                ScoopAbiArgument::direct_parts(
                    aggregate.value_handle().unwrap().canonical_storage(),
                    fixtures::integer_part(
                        aggregate.value_handle().unwrap().canonical_storage(),
                        64,
                    ),
                )
                .unwrap(),
                ScoopAbiArgument::elided_zst(zst.value_handle().unwrap().canonical_storage())
                    .unwrap(),
                ScoopAbiArgument::direct(byte.value_handle().unwrap().canonical_storage()).unwrap(),
            ],
            ScoopAbiReturn::direct_parts(
                aggregate.value_handle().unwrap().canonical_storage(),
                fixtures::integer_part(aggregate.value_handle().unwrap().canonical_storage(), 8),
            )
            .unwrap(),
            (ExactCallableProtocolV1::OrdinaryManaged).gc_effect(),
        )
        .unwrap(),
        &foundation,
    )
    .unwrap();
    let arguments = value.canonical_signature().arguments();
    assert!(matches!(
        arguments,
        [
            ScoopAbiArgument::Direct(_),
            ScoopAbiArgument::ElidedZst(_),
            ScoopAbiArgument::Direct(_),
            ScoopAbiArgument::DirectParts(_, _),
            ScoopAbiArgument::ElidedZst(_),
            ScoopAbiArgument::Direct(_)
        ]
    ));
    assert!(matches!(
        value.canonical_signature().result(),
        ScoopAbiReturn::DirectParts(_, _)
    ));
    fixtures::roundtrip(&value);
    for result in [&unit, &zst, &byte, &reference] {
        let storage = result.value_handle().unwrap().canonical_storage();
        let result_plan = if result.identity().exact() == unit.identity().exact() {
            ScoopAbiReturn::UnitVoid
        } else if storage.byte_size() == 0 {
            ScoopAbiReturn::elided_zst(storage).unwrap()
        } else {
            ScoopAbiReturn::direct(storage).unwrap()
        };
        let value = fixtures::function(result.identity().exact(), result_plan, vec![]);
        match result.value_handle().unwrap().representation().kind() {
            ExactRepresentationKindV1::IntrinsicValue(IntrinsicValueFamilyV1::Unit) => assert_eq!(
                value.canonical_signature().result(),
                ScoopAbiReturn::UnitVoid
            ),
            ExactRepresentationKindV1::Struct(_) => assert!(matches!(
                value.canonical_signature().result(),
                ScoopAbiReturn::ElidedZst(_)
            )),
            _ => assert!(matches!(
                value.canonical_signature().result(),
                ScoopAbiReturn::Direct(_)
            )),
        }
        fixtures::roundtrip(&value);
    }
}

#[test]
fn abi_keeps_integer_coercion_and_pointer_niche_distinct_at_equal_size() {
    let tagged = fixtures::enumeration(&unit());
    let niche = fixtures::enumeration(&managed());
    assert_eq!(
        tagged.value_handle().unwrap().value().storage().byte_size(),
        niche.value_handle().unwrap().value().storage().byte_size()
    );
    let tagged_storage = tagged.value_handle().unwrap().canonical_storage();
    let coercion = fixtures::integer_part(tagged_storage, 64);
    let tagged = fixtures::function(
        tagged.identity().exact(),
        ScoopAbiReturn::direct_parts(tagged_storage, coercion).unwrap(),
        vec![ScoopAbiArgument::direct_parts(tagged_storage, coercion).unwrap()],
    );
    let niche_storage = niche.value_handle().unwrap().canonical_storage();
    let niche = fixtures::function(
        niche.identity().exact(),
        ScoopAbiReturn::direct(niche_storage).unwrap(),
        vec![ScoopAbiArgument::direct(niche_storage).unwrap()],
    );
    assert!(matches!(
        tagged.canonical_signature().arguments(),
        [ScoopAbiArgument::DirectParts(_, _)]
    ));
    assert!(matches!(
        tagged.canonical_signature().result(),
        ScoopAbiReturn::DirectParts(_, _)
    ));
    assert!(matches!(
        niche.canonical_signature().arguments(),
        [ScoopAbiArgument::Direct(_)]
    ));
    assert!(matches!(
        niche.canonical_signature().result(),
        ScoopAbiReturn::Direct(_)
    ));
    fixtures::roundtrip(&tagged);
    fixtures::roundtrip(&niche);
}

#[test]
fn abi_record_requires_its_actual_body_definition() {
    let unit = unit();
    let (target, missing) = fixtures::foundation("invoke", false);
    let signature = CanonicalScoopAbiFunctionSignature::new(
        ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit.identity().exact()),
        vec![],
        ScoopAbiReturn::UnitVoid,
        scoop_identity::GcEffect::NoGc,
    )
    .unwrap();
    assert!(matches!(
        ExactCallableAbiExportV1::from_signature(TARGET, target, signature, &missing),
        Err(ExactCallableAbiError::MissingCallableBody)
    ));
}
