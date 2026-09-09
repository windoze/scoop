use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{DecodedGeneratedNominalKey, GeneratedNominalResolutionError};
use crate::{
    CallableAdapterEnvironmentKey, CallableMaterialization, CallableMaterializationContext,
    CallableTemplateOwner, CborIdentityRecord, ClosureEnvironmentRole, DecodedCborIdentityRecord,
    Effect, ExactCallableSignature, GeneratedNominalIdentityError, GeneratedNominalKey,
    PersistentCallableApplicationId, PersistentConstructorId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentIdMismatch, PersistentIdResolver, PersistentInitializationUnitId,
    PersistentPropertyAccessorId, PersistentTypeId, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
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

test_identity!(PersistentFunctionId);
test_identity!(PersistentGenericFunctionId);
test_identity!(PersistentConstructorId);
test_identity!(PersistentPropertyAccessorId);
test_identity!(PersistentGeneratedCallableId);
test_identity!(PersistentCallableApplicationId);
test_identity!(PersistentInitializationUnitId);
test_identity!(PersistentExactTypeId);
test_identity!(PersistentTypeId);

#[test]
fn all_generated_nominal_shapes_round_trip_and_resolve() {
    let exact = PersistentExactTypeId::expected();
    let materialization = CallableMaterialization::new(
        CallableTemplateOwner::Generated(PersistentGeneratedCallableId::expected()),
        CallableMaterializationContext::Application(PersistentCallableApplicationId::expected()),
    );
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![exact], exact);
    let suspension_site = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::CoroutineTransform, 3),
        [],
    );
    let keys = [
        GeneratedNominalKey::ClosureEnvironment {
            callable: materialization,
            role: ClosureEnvironmentRole::Lambda,
        },
        GeneratedNominalKey::CallableAdapterEnvironment {
            key: CallableAdapterEnvironmentKey::Static {
                source: signature.clone(),
                target: signature.clone(),
            },
        },
        GeneratedNominalKey::CallableAdapterEnvironment {
            key: CallableAdapterEnvironmentKey::Dynamic { target: signature },
        },
        GeneratedNominalKey::CoroutineFrame {
            source_callable: materialization,
        },
        GeneratedNominalKey::ContinuationAdapterEnvironment {
            source_callable: materialization,
            suspension_site,
        },
        GeneratedNominalKey::CoroutineStep { result: exact },
        GeneratedNominalKey::BoxedValue { payload: exact },
        GeneratedNominalKey::CoroutineSlot { value: exact },
        GeneratedNominalKey::ObjectBackingClass {
            object: PersistentTypeId::expected(),
        },
    ];

    for key in keys {
        let decoded = decode_canonical::<DecodedGeneratedNominalKey>(
            &encode(&key).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), key);
    }
}

#[test]
fn generated_nominal_record_resolves_before_recomputing_identity() {
    let key = GeneratedNominalKey::BoxedValue {
        payload: PersistentExactTypeId::expected(),
    };
    let record = CborIdentityRecord::<PersistentTypeId, _>::from_key(key).unwrap();
    let decoded = decode_canonical::<
        DecodedCborIdentityRecord<PersistentTypeId, DecodedGeneratedNominalKey>,
    >(&encode(&record).unwrap(), DecodeLimits::default())
    .unwrap();

    assert_eq!(
        decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
        record
    );
}

#[test]
fn generated_nominal_resolution_rejects_invalid_adapter_target() {
    let exact = PersistentExactTypeId::expected();
    let invalid = GeneratedNominalKey::CallableAdapterEnvironment {
        key: CallableAdapterEnvironmentKey::Dynamic {
            target: ExactCallableSignature::new(Effect::Ordinary, Some(exact), vec![], exact),
        },
    };
    let decoded = decode_canonical::<DecodedGeneratedNominalKey>(
        &encode(&invalid).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(GeneratedNominalResolutionError::Key(
            GeneratedNominalIdentityError::TargetReceiverMustBeAbsent
        ))
    );
}

#[test]
fn generated_nominal_decoder_rejects_unknown_tags() {
    let outer =
        decode_canonical::<DecodedGeneratedNominalKey>(b"\xa1\x00\x09", DecodeLimits::default())
            .unwrap_err();
    assert_eq!(outer.kind(), &WireErrorKind::UnknownTag { tag: 9 });

    let role =
        decode_canonical::<ClosureEnvironmentRole>(b"\x04", DecodeLimits::default()).unwrap_err();
    assert_eq!(role.kind(), &WireErrorKind::UnknownTag { tag: 4 });
}
