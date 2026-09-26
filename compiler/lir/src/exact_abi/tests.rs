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
fn abi_records_preserve_receiver_order_and_all_pass_modes() {
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
            std::iter::once(
                reference
                    .value_handle()
                    .unwrap()
                    .scoop_abi_argument(TARGET)
                    .unwrap(),
            )
            .chain(parameters.iter().map(|layout| {
                layout
                    .value_handle()
                    .unwrap()
                    .scoop_abi_argument(TARGET)
                    .unwrap()
            }))
            .collect(),
            aggregate
                .value_handle()
                .unwrap()
                .scoop_abi_return(TARGET)
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
            ScoopAbiArgument::Indirect(_),
            ScoopAbiArgument::ElidedZst(_),
            ScoopAbiArgument::Direct(_)
        ]
    ));
    assert!(matches!(
        value.canonical_signature().result(),
        ScoopAbiReturn::Indirect(_)
    ));
    fixtures::roundtrip(&value);
    for result in [&unit, &zst, &byte, &reference] {
        let value = fixtures::function(result, &[]);
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
fn abi_classifies_tagged_enum_as_indirect_and_pointer_niche_as_direct_at_equal_size() {
    let tagged = fixtures::enumeration(&unit());
    let niche = fixtures::enumeration(&managed());
    assert_eq!(
        tagged.value_handle().unwrap().value().storage().byte_size(),
        niche.value_handle().unwrap().value().storage().byte_size()
    );
    let tagged = fixtures::function(&tagged, &[&tagged]);
    let niche = fixtures::function(&niche, &[&niche]);
    assert!(matches!(
        tagged.canonical_signature().arguments(),
        [ScoopAbiArgument::Indirect(_)]
    ));
    assert!(matches!(
        tagged.canonical_signature().result(),
        ScoopAbiReturn::Indirect(_)
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
