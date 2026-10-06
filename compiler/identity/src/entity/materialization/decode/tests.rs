use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::{DecodedInitializationUnitKey, DecodedLocalValueKey};
use crate::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CborIdentityRecord, DecodedCborIdentityRecord, DecodedPersistentId, InitializationUnitKey,
    LocalValueKey, LocalValueSelector, NonEmptyVec, PersistentCallableApplicationId,
    PersistentConstructorId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentGenericFunctionId, PersistentIdMismatch, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentLocalValueId, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment, SyntheticLocalRole,
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

            fn resolve(&mut self, id: DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify(<$id>::expected())
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

test_identity!(PersistentPropertyId);
test_identity!(PersistentExtensionPropertyId);
test_identity!(PersistentTypeId);
test_identity!(PersistentExactTypeId);
test_identity!(PersistentFunctionId);
test_identity!(PersistentGenericFunctionId);
test_identity!(crate::PersistentGenericTypeId);
test_identity!(PersistentConstructorId);
test_identity!(PersistentPropertyAccessorId);
test_identity!(PersistentGeneratedCallableId);
test_identity!(PersistentCallableApplicationId);
test_identity!(PersistentInitializationUnitId);
test_identity!(PersistentEnumVariantId);

#[test]
fn all_initialization_unit_records_round_trip_and_resolve() {
    let property = PersistentPropertyId::expected();
    let extension = PersistentExtensionPropertyId::expected();
    let exact = PersistentExactTypeId::expected();
    let keys = [
        InitializationUnitKey::TopLevelProperty(property),
        InitializationUnitKey::ExtensionProperty(extension),
        InitializationUnitKey::Object(PersistentTypeId::expected()),
        InitializationUnitKey::Companion(PersistentTypeId::expected()),
        InitializationUnitKey::GenericDelegatedExtensionApplication {
            property: extension,
            receiver_arguments: NonEmptyVec::from_first(exact, [exact]),
        },
    ];

    for key in keys {
        let record =
            CborIdentityRecord::<PersistentInitializationUnitId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentInitializationUnitId, DecodedInitializationUnitKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn all_local_value_selectors_round_trip_and_resolve() {
    let path = StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::SyntheticValue, 4),
        [],
    );
    let mut selectors = vec![
        LocalValueSelector::This,
        LocalValueSelector::Parameter {
            declaration_index: 3,
        },
        LocalValueSelector::LocalDeclaration { path: path.clone() },
        LocalValueSelector::BoundReceiver { path: path.clone() },
        LocalValueSelector::SuspensionResult { site: path.clone() },
    ];
    selectors.extend(
        [
            SyntheticLocalRole::Temporary,
            SyntheticLocalRole::DefaultValue,
            SyntheticLocalRole::DesugaredIterator,
            SyntheticLocalRole::CoroutineProtocol,
            SyntheticLocalRole::CallbackContext,
        ]
        .into_iter()
        .map(|role| LocalValueSelector::Synthetic {
            path: path.clone(),
            role,
        }),
    );
    let owner = CallableMaterialization::new(
        CallableTemplateOwner::Generated(PersistentGeneratedCallableId::expected()),
        CallableMaterializationContext::Application(PersistentCallableApplicationId::expected()),
    );

    for selector in selectors {
        let record = CborIdentityRecord::<PersistentLocalValueId, _>::from_key(LocalValueKey::new(
            owner, selector,
        ))
        .unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentLocalValueId, DecodedLocalValueKey>,
        >(&encode(&record).unwrap())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn materialization_decoder_rejects_empty_arguments_and_unknown_tags() {
    let mut empty = vec![0xa3, 0x00, 0x05, 0x01, 0x58, 0x20];
    empty.extend_from_slice(PersistentExtensionPropertyId::expected().as_array());
    empty.extend_from_slice(&[0x02, 0x80]);
    let error = decode_canonical::<DecodedInitializationUnitKey>(&empty).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 0,
        }
    );

    let unknown = decode_canonical::<DecodedInitializationUnitKey>(b"\xa1\x00\x08").unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 8 });

    let selector = decode_error_kind::<LocalValueSelector>(b"\xa1\x00\x07");
    assert_eq!(selector, WireErrorKind::UnknownTag { tag: 7 });

    let role = decode_canonical::<SyntheticLocalRole>(b"\x06").unwrap_err();
    assert_eq!(role.kind(), &WireErrorKind::UnknownTag { tag: 6 });
}

fn decode_error_kind<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8]) -> WireErrorKind {
    decode_canonical::<T>(bytes).unwrap_err().kind().clone()
}
