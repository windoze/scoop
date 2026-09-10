use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    CallableApplicationResolutionError, DecodedCallableApplicationKey, DecodedCallableArguments,
    DecodedCallableMaterialization, DecodedPropertyAccessorKey,
};
use crate::{
    AccessorRole, CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
    CallableMaterializationContext, CallableTemplateOwner, CborIdentityRecord,
    DecodedCborIdentityRecord, NonEmptyVec, PersistentCallableApplicationId,
    PersistentConstructorId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentIdMismatch, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentPropertyAccessorId, PersistentPropertyId,
    PropertyAccessorKey, PropertyOwner,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the test graph")
    }
}

impl std::error::Error for ResolutionError {}

struct Resolver;

trait TestId {
    fn expected() -> Self;
}

macro_rules! test_identity {
    ($id:ty) => {
        impl TestId for $id {
            fn expected() -> Self {
                Self([7; 32])
            }
        }

        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify(<$id>::expected())
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

test_identity!(PersistentPropertyId);
test_identity!(PersistentExtensionPropertyId);
test_identity!(PersistentFunctionId);
test_identity!(PersistentGenericFunctionId);
test_identity!(PersistentConstructorId);
test_identity!(PersistentPropertyAccessorId);
test_identity!(PersistentExactTypeId);
test_identity!(PersistentCallableApplicationId);
test_identity!(PersistentInitializationUnitId);
test_identity!(PersistentGeneratedCallableId);
test_identity!(PersistentEnumVariantId);

#[test]
fn property_accessor_record_resolves_owner_before_recomputing_identity() {
    let key = PropertyAccessorKey::new(
        PropertyOwner::Property(PersistentPropertyId::expected()),
        AccessorRole::Setter,
    );
    let record = CborIdentityRecord::<PersistentPropertyAccessorId, _>::from_key(key).unwrap();
    let bytes = encode(&record).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentPropertyAccessorId, DecodedPropertyAccessorKey>,
    >(&bytes, DecodeLimits::default())
    .unwrap();

    assert_eq!(
        decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
        record
    );
}

#[test]
fn all_callable_application_shapes_round_trip_and_resolve() {
    let exact = PersistentExactTypeId::expected();
    let owner = CallableInstantiationOwner::ExactNominalOwner(exact);
    let applications = [
        CallableApplicationKey::for_function(PersistentFunctionId::expected(), owner),
        CallableApplicationKey::for_generic_function(
            PersistentGenericFunctionId::expected(),
            owner,
            NonEmptyVec::from_first(exact, []),
        ),
        CallableApplicationKey::for_constructor(PersistentConstructorId::expected(), owner),
        CallableApplicationKey::for_accessor(PersistentPropertyAccessorId::expected(), owner),
        CallableApplicationKey::for_variant_constructor(PersistentEnumVariantId::expected(), owner),
        CallableApplicationKey::for_generic_extension_accessor(
            PersistentPropertyAccessorId::expected(),
            owner,
            NonEmptyVec::from_first(exact, []),
        ),
    ];

    for application in applications {
        let decoded = decode_canonical::<DecodedCallableApplicationKey>(
            &encode(&application).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), application);
    }
}

#[test]
fn callable_application_rejects_template_argument_shape_mismatches() {
    let key = CallableApplicationKey::for_function(
        PersistentFunctionId::expected(),
        CallableInstantiationOwner::NoOwner,
    );
    let mut decoded = decode_canonical::<DecodedCallableApplicationKey>(
        &encode(&key).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    decoded.callable_arguments = DecodedCallableArguments::Arguments(NonEmptyVec::from_first(
        crate::DecodedPersistentId::from_unvalidated_bytes(
            *PersistentExactTypeId::expected().as_array(),
        ),
        [],
    ));

    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(CallableApplicationResolutionError::Shape)
    );
}

#[test]
fn callable_materialization_round_trips_template_and_context() {
    let materialization = CallableMaterialization::new(
        CallableTemplateOwner::VariantConstructor(PersistentEnumVariantId::expected()),
        CallableMaterializationContext::InitializationApplication(
            PersistentInitializationUnitId::expected(),
        ),
    );
    let decoded = decode_canonical::<DecodedCallableMaterialization>(
        &encode(&materialization).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), materialization);
}

#[test]
fn callable_decoder_rejects_empty_argument_lists_and_unknown_roles() {
    let empty_arguments = decode_canonical::<DecodedCallableArguments>(
        b"\xa2\x00\x02\x01\x80",
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        empty_arguments.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 0,
        }
    );

    let property = PersistentPropertyId::expected();
    let mut bad_role = vec![0xa2, 0x01, 0xa2, 0x00, 0x01, 0x01, 0x58, 0x20];
    bad_role.extend_from_slice(property.as_array());
    bad_role.extend_from_slice(&[0x02, 0x03]);
    let error = decode_canonical::<DecodedPropertyAccessorKey>(&bad_role, DecodeLimits::default())
        .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}
