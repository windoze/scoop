use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{DecodedSourceContextKey, SourceContextResolutionError};
use crate::{
    CallableOwner, CborIdentityRecord, ConeIdentity, DecodedCborIdentityRecord,
    DecodedPersistentId, NominalDeclarationOwner, NormalizedSourcePath,
    PersistentCallableApplicationId, PersistentConstructorId, PersistentExtensionPropertyId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentInitializationUnitId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentSourceContextId, PersistentTypeId, PropertyOwner, SourceContextKey, SourceIdentity,
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

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE)
            .map_err(|_: PersistentIdMismatch<ConeIdentity>| ResolutionError)
    }
}

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

test_identity!(PersistentTypeId);
test_identity!(PersistentGenericTypeId);
test_identity!(PersistentFunctionId);
test_identity!(PersistentGenericFunctionId);
test_identity!(PersistentCallableApplicationId);
test_identity!(PersistentConstructorId);
test_identity!(PersistentPropertyAccessorId);
test_identity!(PersistentGeneratedCallableId);
test_identity!(PersistentPropertyId);
test_identity!(PersistentExtensionPropertyId);
test_identity!(PersistentInitializationUnitId);

#[test]
fn all_source_context_records_round_trip_and_resolve() {
    let source = source();
    let keys = [
        SourceContextKey::File {
            source: source.clone(),
        },
        SourceContextKey::Nominal {
            source: source.clone(),
            owner: NominalDeclarationOwner::Concrete(PersistentTypeId::expected()),
        },
        SourceContextKey::Callable {
            source: source.clone(),
            owner: CallableOwner::Generated(PersistentGeneratedCallableId::expected()),
        },
        SourceContextKey::Property {
            source: source.clone(),
            owner: PropertyOwner::ExtensionProperty(PersistentExtensionPropertyId::expected()),
        },
        SourceContextKey::Initialization {
            source,
            unit: PersistentInitializationUnitId::expected(),
        },
    ];

    for key in keys {
        let record = CborIdentityRecord::<PersistentSourceContextId, _>::from_key(key).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentSourceContextId, DecodedSourceContextKey>,
        >(&encode(&record).unwrap(), DecodeLimits::default())
        .unwrap();
        assert_eq!(
            decoded.resolve(|key| key.resolve(&mut Resolver)).unwrap(),
            record
        );
    }
}

#[test]
fn source_context_resolution_rejects_an_unknown_owner_reference() {
    let key = SourceContextKey::Nominal {
        source: source(),
        owner: NominalDeclarationOwner::Concrete(PersistentTypeId::expected()),
    };
    let mut bytes = encode(&key).unwrap();
    assert_eq!(*bytes.last().unwrap(), 7);
    *bytes.last_mut().unwrap() = 8;
    let decoded =
        decode_canonical::<DecodedSourceContextKey>(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(SourceContextResolutionError::Reference(ResolutionError))
    );
}

#[test]
fn source_context_decoder_rejects_unknown_and_incomplete_variants() {
    let unknown = decode_canonical::<DecodedSourceContextKey>(
        b"\xa2\x00\x06\x01\xa2\x01\x58\x20\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x61x",
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(unknown.kind(), &WireErrorKind::UnknownTag { tag: 6 });

    let incomplete = decode_canonical::<DecodedSourceContextKey>(
        b"\xa2\x00\x02\x01\xa2\x01\x58\x20\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x61x",
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        incomplete.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 2,
        }
    );
}

fn source() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/context.scoop").unwrap(),
    )
    .unwrap()
}
