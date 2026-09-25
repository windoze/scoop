use scoop_identity::*;
use scoop_wire::{WireEncode, decode_canonical, encode};

use super::*;
use crate::exact_layout::tests::{Bound, exact, field, integer, managed, source, unit};
use crate::*;

mod common;
mod fixtures;
mod physical;
mod reader;
mod shared_record;
mod table;

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

#[test]
fn abi_replays_receiver_order_repeated_layouts_and_all_pass_modes() {
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
    let value = ExactCallableAbiExportV1::replay(
        TARGET,
        target,
        signature,
        ExactCallableProtocolV1::OrdinaryManaged,
        CallableAbiLayoutInputsV1 {
            receiver: CallableAbiReceiverInputV1::Receiver(&reference),
            parameters: &parameters,
            result: &aggregate,
        },
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
    assert_eq!(
        value.layout_dependencies().parameters()[1]
            .identity()
            .layout(),
        value.layout_dependencies().parameters()[4]
            .identity()
            .layout()
    );
    assert!(Arc::ptr_eq(
        &value.layout_dependencies().parameters()[1],
        &value.layout_dependencies().parameters()[4]
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
fn abi_rejects_missing_receiver_wrong_exact_role_and_body() {
    let unit: ExactLayoutExportV1 = unit().into();
    let byte: ExactLayoutExportV1 = integer("Byte", IntegerKind::SIGNED_8).into();
    let (target, foundation) = fixtures::foundation("invoke", true);
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(byte.identity().exact()),
        vec![],
        unit.identity().exact(),
    );
    assert!(matches!(
        ExactCallableAbiExportV1::replay(
            TARGET,
            target,
            signature,
            ExactCallableProtocolV1::OrdinaryNoGc,
            CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::NoReceiver,
                parameters: &[],
                result: &unit
            },
            &foundation
        ),
        Err(ExactCallableAbiError::Receiver)
    ));
    let signature = ExactCallableSignature::new(
        Effect::Ordinary,
        None,
        vec![byte.identity().exact()],
        unit.identity().exact(),
    );
    assert!(matches!(
        ExactCallableAbiExportV1::replay(
            TARGET,
            target,
            signature,
            ExactCallableProtocolV1::OrdinaryNoGc,
            CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::NoReceiver,
                parameters: &[&unit],
                result: &unit
            },
            &foundation
        ),
        Err(ExactCallableAbiError::ExactType)
    ));
    let bound = Bound::instance(exact(&source("Class", SourceNominalKind::Class, 0)));
    let instance: ExactLayoutExportV1 =
        ExactInstanceLayoutV1::abstract_reference(bound.identity, &bound.foundation)
            .unwrap()
            .into();
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, vec![], instance.identity().exact());
    assert!(matches!(
        ExactCallableAbiExportV1::replay(
            TARGET,
            target,
            signature,
            ExactCallableProtocolV1::OrdinaryNoGc,
            CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::NoReceiver,
                parameters: &[],
                result: &instance
            },
            &foundation
        ),
        Err(ExactCallableAbiError::LayoutRole)
    ));
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit.identity().exact());
    let (_, missing) = fixtures::foundation("invoke", false);
    assert!(matches!(
        ExactCallableAbiExportV1::replay(
            TARGET,
            target,
            signature.clone(),
            ExactCallableProtocolV1::OrdinaryNoGc,
            CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::NoReceiver,
                parameters: &[],
                result: &unit
            },
            &missing
        ),
        Err(ExactCallableAbiError::MissingCallableBody)
    ));
}
