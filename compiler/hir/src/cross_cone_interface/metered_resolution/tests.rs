use super::*;
use crate::{
    CanonicalBinderListV1, CanonicalSourceParameterShapesV1, DecodedCanonicalBinderListV1,
    DecodedCanonicalSourceParameterShapesV1, SourceParameterShapeV1, TypeParameterBinderV1,
    TypeParameterBoundsV1,
};
use scoop_identity::{CanonicalIdentifier, PendingIdentityValidation, SignatureTypeKey};
use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

#[test]
fn aggregate_source_readers_charge_names_collections_and_signature_trees() {
    let binders = CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
        CanonicalIdentifier::new("T").unwrap(),
        TypeParameterBoundsV1::Unconstrained,
    )])
    .unwrap();
    let parameters = CanonicalSourceParameterShapesV1::try_new(vec![SourceParameterShapeV1::new(
        CanonicalIdentifier::new("value").unwrap(),
        SignatureTypeKey::RawPointer(Box::new(SignatureTypeKey::Binder { depth: 0, index: 0 })),
    )])
    .unwrap();
    let decoded_binders: DecodedCanonicalBinderListV1 =
        decode_canonical(&encode(&binders).unwrap(), DecodeLimits::default()).unwrap();
    let decoded_parameters: DecodedCanonicalSourceParameterShapesV1 =
        decode_canonical(&encode(&parameters).unwrap(), DecodeLimits::default()).unwrap();
    let mut authority = PendingIdentityValidation::new().finish().unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    assert_eq!(
        decoded_binders
            .clone()
            .resolve_metered(&mut authority, &mut meter)
            .unwrap(),
        binders
    );
    assert_eq!(
        decoded_parameters
            .clone()
            .resolve_metered(&mut authority, &mut meter)
            .unwrap(),
        parameters
    );
    assert_eq!(meter.usage().owned_bytes, 12);
    let limited = || {
        BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        })
    };
    assert!(matches!(
        decoded_binders.resolve_metered(&mut authority, &mut limited()),
        Err(MeteredInterfaceResolutionError::Resource(_))
    ));
    assert!(matches!(
        decoded_parameters.resolve_metered(&mut authority, &mut limited()),
        Err(MeteredInterfaceResolutionError::Resource(_))
    ));
}
