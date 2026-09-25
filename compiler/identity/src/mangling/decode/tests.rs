use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedManglingSchemaIdentity, DecodedPersistentSymbolKey, DecodedPersistentSymbolRequest,
    DecodedPersistentSymbolRequestTable, ManglingSchemaIdentityError,
    PersistentSymbolResolutionError,
};
use crate::{
    ConeIdentity, GeneratedBridgeAtomId, LinkageClass, ManglingSchemaIdentity,
    ObjectDefinitionAtomId, OdrMemberId, PersistentCallableBodyId, PersistentDispatchSlotId,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentLayoutId,
    PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId, PersistentSymbolError,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

macro_rules! id_resolver {
    ($id:ty) => {
        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify(<$id>::expected())
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

trait TestId {
    fn expected() -> Self;
}

macro_rules! test_id {
    ($id:ty) => {
        impl TestId for $id {
            fn expected() -> Self {
                Self([7; 32])
            }
        }

        id_resolver!($id);
    };
}

test_id!(PersistentCallableBodyId);
test_id!(PersistentStaticStorageId);
test_id!(PersistentImmortalObjectId);
test_id!(PersistentExactTypeId);
test_id!(PersistentLayoutId);
test_id!(PersistentScanId);
test_id!(PersistentDispatchTableId);
test_id!(PersistentDispatchSlotId);
test_id!(PersistentInitializationUnitId);
test_id!(PersistentSafepointSiteId);
test_id!(ConeIdentity);
test_id!(GeneratedBridgeAtomId);
test_id!(OdrMemberId);
test_id!(ObjectDefinitionAtomId);

#[test]
fn the_only_mangling_schema_identity_round_trips_and_rejects_other_names() {
    let decoded = decode_canonical::<DecodedManglingSchemaIdentity>(
        &encode(&ManglingSchemaIdentity).unwrap(),
    )
    .unwrap();
    assert_eq!(decoded.resolve().unwrap(), ManglingSchemaIdentity);

    let decoded = decode_canonical::<DecodedManglingSchemaIdentity>(b"\x6dunknown-value").unwrap();
    assert_eq!(
        decoded.resolve(),
        Err(ManglingSchemaIdentityError::UnknownIdentity(
            "unknown-value".to_string()
        ))
    );
}

#[test]
fn every_symbol_key_round_trips_and_resolves_its_typed_owner() {
    for key in symbol_keys() {
        let decoded =
            decode_canonical::<DecodedPersistentSymbolKey>(&encode(&key).unwrap()).unwrap();

        assert_eq!(decoded.kind(), key.kind());
        assert_eq!(decoded.owner_bytes(), key.owner_bytes());
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), key);
    }
}

#[test]
fn requests_round_trip_and_recheck_the_linkage_matrix() {
    for key in symbol_keys() {
        let linkage = if key.kind() == crate::PersistentSymbolKind::OdrMember {
            LinkageClass::OdrWeak
        } else {
            LinkageClass::ConeStrong
        };
        let request = PersistentSymbolRequest::new(key, linkage).unwrap();
        let decoded =
            decode_canonical::<DecodedPersistentSymbolRequest>(&encode(&request).unwrap()).unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), request);
    }

    let invalid = RawRequest {
        key: PersistentSymbolKey::CallableBody(PersistentCallableBodyId::expected()),
        linkage: LinkageClass::RuntimeAbi,
    };
    let decoded =
        decode_canonical::<DecodedPersistentSymbolRequest>(&encode(&invalid).unwrap()).unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(PersistentSymbolResolutionError::Symbol(
            PersistentSymbolError::LinkageNotAllowed {
                kind: crate::PersistentSymbolKind::CallableBody,
                linkage: LinkageClass::RuntimeAbi,
            }
        ))
    );
}

#[test]
fn canonical_request_table_round_trips_without_reader_side_sorting() {
    let requests = canonical_requests();
    let table = PersistentSymbolRequestTable::new(requests).unwrap();
    let decoded =
        decode_canonical::<DecodedPersistentSymbolRequestTable>(&encode(&table).unwrap()).unwrap();

    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), table);
}

#[test]
fn request_table_rejects_noncanonical_order_and_duplicates() {
    let requests = canonical_requests();
    let reversed = RawTable(requests.iter().copied().rev().collect());
    let decoded =
        decode_canonical::<DecodedPersistentSymbolRequestTable>(&encode(&reversed).unwrap())
            .unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver),
        Err(PersistentSymbolResolutionError::NonCanonicalRequestOrder { .. })
    ));

    let duplicate = RawTable(vec![requests[0], requests[0]]);
    let decoded =
        decode_canonical::<DecodedPersistentSymbolRequestTable>(&encode(&duplicate).unwrap())
            .unwrap();
    assert_eq!(
        decoded.resolve(&mut Resolver),
        Err(PersistentSymbolResolutionError::DuplicateRequest {
            kind: requests[0].key().kind()
        })
    );
}

#[test]
fn symbol_decoder_rejects_unknown_tags_and_unregistered_references() {
    let mut unknown_key = vec![0xa2, 0x00, 0x17, 0x01, 0x58, 0x20];
    unknown_key.extend_from_slice(&[7; 32]);
    let error = decode_canonical::<DecodedPersistentSymbolKey>(&unknown_key).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 23 });

    let error = decode_canonical::<LinkageClass>(b"\x05").unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let wrong = PersistentSymbolKey::CallableBody(PersistentCallableBodyId([99; 32]));
    let decoded = decode_canonical::<DecodedPersistentSymbolKey>(&encode(&wrong).unwrap()).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver), Err(ResolutionError));
}

fn canonical_requests() -> Vec<PersistentSymbolRequest> {
    vec![
        PersistentSymbolRequest::new(
            PersistentSymbolKey::CallableBody(PersistentCallableBodyId::expected()),
            LinkageClass::ConeStrong,
        )
        .unwrap(),
        PersistentSymbolRequest::new(
            PersistentSymbolKey::StaticStorage(PersistentStaticStorageId::expected()),
            LinkageClass::TemplateSupportHidden,
        )
        .unwrap(),
        PersistentSymbolRequest::new(
            PersistentSymbolKey::OdrMember(OdrMemberId::expected()),
            LinkageClass::OdrWeak,
        )
        .unwrap(),
    ]
}

fn symbol_keys() -> [PersistentSymbolKey; 22] {
    [
        PersistentSymbolKey::CallableBody(PersistentCallableBodyId::expected()),
        PersistentSymbolKey::StaticStorage(PersistentStaticStorageId::expected()),
        PersistentSymbolKey::ImmortalObject(PersistentImmortalObjectId::expected()),
        PersistentSymbolKey::TypeDescriptor(PersistentExactTypeId::expected()),
        PersistentSymbolKey::Layout(PersistentLayoutId::expected()),
        PersistentSymbolKey::ScanProgram(PersistentScanId::expected()),
        PersistentSymbolKey::DispatchTable(PersistentDispatchTableId::expected()),
        PersistentSymbolKey::DispatchSlot(PersistentDispatchSlotId::expected()),
        PersistentSymbolKey::InitializationCell(PersistentInitializationUnitId::expected()),
        PersistentSymbolKey::InitializationDescriptor(PersistentInitializationUnitId::expected()),
        PersistentSymbolKey::RootRegistration(PersistentStaticStorageId::expected()),
        PersistentSymbolKey::ImmortalRegistration(PersistentImmortalObjectId::expected()),
        PersistentSymbolKey::InitializationRegistration(PersistentInitializationUnitId::expected()),
        PersistentSymbolKey::TypeRegistration(PersistentExactTypeId::expected()),
        PersistentSymbolKey::SafepointRegistration(PersistentSafepointSiteId::expected()),
        PersistentSymbolKey::CallableRegistration(PersistentCallableBodyId::expected()),
        PersistentSymbolKey::ImageDescriptor(ConeIdentity::expected()),
        PersistentSymbolKey::GeneratedBridge(GeneratedBridgeAtomId::expected()),
        PersistentSymbolKey::OdrMember(OdrMemberId::expected()),
        PersistentSymbolKey::DefinitionBoundaryStart(ObjectDefinitionAtomId::expected()),
        PersistentSymbolKey::DefinitionBoundaryEnd(ObjectDefinitionAtomId::expected()),
        PersistentSymbolKey::RootEntryDescriptor(ConeIdentity::expected()),
    ]
}

struct RawRequest {
    key: PersistentSymbolKey,
    linkage: LinkageClass,
}

impl WireEncode for RawRequest {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.key.encode(encoder)?;
        encoder.field(2)?;
        self.linkage.encode(encoder)
    }
}

struct RawTable(Vec<PersistentSymbolRequest>);

impl WireEncode for RawTable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for request in &self.0 {
            request.encode(encoder)?;
        }
        Ok(())
    }
}
