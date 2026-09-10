use std::fmt;

use scoop_wire::{Encoder, WireEncode};

use crate::{
    ConeIdentity, GeneratedBridgeAtomId, ObjectDefinitionAtomId, OdrMemberId,
    PersistentCallableBodyId, PersistentDispatchSlotId, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
};

mod decode;

pub use decode::{
    DecodedManglingSchemaIdentity, DecodedPersistentSymbolKey, DecodedPersistentSymbolRequest,
    DecodedPersistentSymbolRequestTable, ManglingSchemaIdentityError,
    PersistentSymbolResolutionError, PersistentSymbolResolver,
};

const MANGLED_SYMBOL_PREFIX: &str = "scoop$1$";
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

const MANGLING_SCHEMA_NAME: &str = "persistent-v1";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ManglingSchemaIdentity;

impl ManglingSchemaIdentity {
    pub const fn canonical_name(self) -> &'static str {
        MANGLING_SCHEMA_NAME
    }
}

impl WireEncode for ManglingSchemaIdentity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(self.canonical_name())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PersistentSymbolKind {
    CallableBody,
    StaticStorage,
    ImmortalObject,
    TypeDescriptor,
    Layout,
    ScanProgram,
    DispatchTable,
    DispatchSlot,
    InitializationCell,
    InitializationDescriptor,
    RootRegistration,
    ImmortalRegistration,
    InitializationRegistration,
    TypeRegistration,
    SafepointRegistration,
    CallableRegistration,
    ImageDescriptor,
    GeneratedBridge,
    OdrMember,
    DefinitionBoundaryStart,
    DefinitionBoundaryEnd,
}

impl PersistentSymbolKind {
    pub const fn tag(self) -> u64 {
        match self {
            Self::CallableBody => 1,
            Self::StaticStorage => 2,
            Self::ImmortalObject => 3,
            Self::TypeDescriptor => 4,
            Self::Layout => 5,
            Self::ScanProgram => 6,
            Self::DispatchTable => 7,
            Self::DispatchSlot => 8,
            Self::InitializationCell => 9,
            Self::InitializationDescriptor => 10,
            Self::RootRegistration => 11,
            Self::ImmortalRegistration => 12,
            Self::InitializationRegistration => 13,
            Self::TypeRegistration => 14,
            Self::SafepointRegistration => 15,
            Self::CallableRegistration => 16,
            Self::ImageDescriptor => 17,
            Self::GeneratedBridge => 18,
            Self::OdrMember => 19,
            Self::DefinitionBoundaryStart => 20,
            Self::DefinitionBoundaryEnd => 21,
        }
    }

    pub const fn symbol_tag(self) -> &'static str {
        match self {
            Self::CallableBody => "cb",
            Self::StaticStorage => "ss",
            Self::ImmortalObject => "io",
            Self::TypeDescriptor => "td",
            Self::Layout => "ly",
            Self::ScanProgram => "sp",
            Self::DispatchTable => "dt",
            Self::DispatchSlot => "ds",
            Self::InitializationCell => "ic",
            Self::InitializationDescriptor => "id",
            Self::RootRegistration => "rr",
            Self::ImmortalRegistration => "ir",
            Self::InitializationRegistration => "nr",
            Self::TypeRegistration => "tr",
            Self::SafepointRegistration => "sr",
            Self::CallableRegistration => "cr",
            Self::ImageDescriptor => "im",
            Self::GeneratedBridge => "br",
            Self::OdrMember => "od",
            Self::DefinitionBoundaryStart => "bs",
            Self::DefinitionBoundaryEnd => "be",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PersistentSymbolKey {
    CallableBody(PersistentCallableBodyId),
    StaticStorage(PersistentStaticStorageId),
    ImmortalObject(PersistentImmortalObjectId),
    TypeDescriptor(PersistentExactTypeId),
    Layout(PersistentLayoutId),
    ScanProgram(PersistentScanId),
    DispatchTable(PersistentDispatchTableId),
    DispatchSlot(PersistentDispatchSlotId),
    InitializationCell(PersistentInitializationUnitId),
    InitializationDescriptor(PersistentInitializationUnitId),
    RootRegistration(PersistentStaticStorageId),
    ImmortalRegistration(PersistentImmortalObjectId),
    InitializationRegistration(PersistentInitializationUnitId),
    TypeRegistration(PersistentExactTypeId),
    SafepointRegistration(PersistentSafepointSiteId),
    CallableRegistration(PersistentCallableBodyId),
    ImageDescriptor(ConeIdentity),
    GeneratedBridge(GeneratedBridgeAtomId),
    OdrMember(OdrMemberId),
    DefinitionBoundaryStart(ObjectDefinitionAtomId),
    DefinitionBoundaryEnd(ObjectDefinitionAtomId),
}

impl PersistentSymbolKey {
    pub const fn kind(self) -> PersistentSymbolKind {
        match self {
            Self::CallableBody(_) => PersistentSymbolKind::CallableBody,
            Self::StaticStorage(_) => PersistentSymbolKind::StaticStorage,
            Self::ImmortalObject(_) => PersistentSymbolKind::ImmortalObject,
            Self::TypeDescriptor(_) => PersistentSymbolKind::TypeDescriptor,
            Self::Layout(_) => PersistentSymbolKind::Layout,
            Self::ScanProgram(_) => PersistentSymbolKind::ScanProgram,
            Self::DispatchTable(_) => PersistentSymbolKind::DispatchTable,
            Self::DispatchSlot(_) => PersistentSymbolKind::DispatchSlot,
            Self::InitializationCell(_) => PersistentSymbolKind::InitializationCell,
            Self::InitializationDescriptor(_) => PersistentSymbolKind::InitializationDescriptor,
            Self::RootRegistration(_) => PersistentSymbolKind::RootRegistration,
            Self::ImmortalRegistration(_) => PersistentSymbolKind::ImmortalRegistration,
            Self::InitializationRegistration(_) => PersistentSymbolKind::InitializationRegistration,
            Self::TypeRegistration(_) => PersistentSymbolKind::TypeRegistration,
            Self::SafepointRegistration(_) => PersistentSymbolKind::SafepointRegistration,
            Self::CallableRegistration(_) => PersistentSymbolKind::CallableRegistration,
            Self::ImageDescriptor(_) => PersistentSymbolKind::ImageDescriptor,
            Self::GeneratedBridge(_) => PersistentSymbolKind::GeneratedBridge,
            Self::OdrMember(_) => PersistentSymbolKind::OdrMember,
            Self::DefinitionBoundaryStart(_) => PersistentSymbolKind::DefinitionBoundaryStart,
            Self::DefinitionBoundaryEnd(_) => PersistentSymbolKind::DefinitionBoundaryEnd,
        }
    }

    pub fn owner_bytes(&self) -> &[u8; 32] {
        match self {
            Self::CallableBody(id) | Self::CallableRegistration(id) => id.as_array(),
            Self::StaticStorage(id) | Self::RootRegistration(id) => id.as_array(),
            Self::ImmortalObject(id) | Self::ImmortalRegistration(id) => id.as_array(),
            Self::TypeDescriptor(id) | Self::TypeRegistration(id) => id.as_array(),
            Self::Layout(id) => id.as_array(),
            Self::ScanProgram(id) => id.as_array(),
            Self::DispatchTable(id) => id.as_array(),
            Self::DispatchSlot(id) => id.as_array(),
            Self::InitializationCell(id)
            | Self::InitializationDescriptor(id)
            | Self::InitializationRegistration(id) => id.as_array(),
            Self::SafepointRegistration(id) => id.as_array(),
            Self::ImageDescriptor(id) => id.as_array(),
            Self::GeneratedBridge(id) => id.as_array(),
            Self::OdrMember(id) => id.as_array(),
            Self::DefinitionBoundaryStart(id) | Self::DefinitionBoundaryEnd(id) => id.as_array(),
        }
    }
}

impl WireEncode for PersistentSymbolKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(self.kind().tag())?;
        encoder.field(1)?;
        encoder.bytes(self.owner_bytes())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MangledSymbol(String);

impl MangledSymbol {
    pub fn from_key(key: &PersistentSymbolKey) -> Self {
        let mut symbol = String::with_capacity(MANGLED_SYMBOL_PREFIX.len() + 2 + 1 + 64);
        symbol.push_str(MANGLED_SYMBOL_PREFIX);
        symbol.push_str(key.kind().symbol_tag());
        symbol.push('$');
        for byte in key.owner_bytes() {
            symbol.push(HEX_DIGITS[usize::from(byte >> 4)] as char);
            symbol.push(HEX_DIGITS[usize::from(byte & 0x0f)] as char);
        }
        Self(symbol)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MangledSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LinkageClass {
    ConeStrong,
    TemplateSupportHidden,
    OdrWeak,
    RuntimeAbi,
}

impl WireEncode for LinkageClass {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ConeStrong => 1,
            Self::TemplateSupportHidden => 2,
            Self::OdrWeak => 3,
            Self::RuntimeAbi => 4,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PersistentSymbolRequest {
    key: PersistentSymbolKey,
    linkage: LinkageClass,
}

impl PersistentSymbolRequest {
    pub fn new(
        key: PersistentSymbolKey,
        linkage: LinkageClass,
    ) -> Result<Self, PersistentSymbolError> {
        if !linkage_allowed_by_kind(key.kind(), linkage) {
            return Err(PersistentSymbolError::LinkageNotAllowed {
                kind: key.kind(),
                linkage,
            });
        }
        Ok(Self { key, linkage })
    }

    pub const fn key(self) -> PersistentSymbolKey {
        self.key
    }

    pub const fn linkage(self) -> LinkageClass {
        self.linkage
    }

    pub fn symbol(&self) -> MangledSymbol {
        MangledSymbol::from_key(&self.key)
    }
}

impl WireEncode for PersistentSymbolRequest {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.key.encode(encoder)?;
        encoder.field(2)?;
        self.linkage.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentSymbolRequestTable {
    requests: Vec<PersistentSymbolRequest>,
}

impl PersistentSymbolRequestTable {
    pub const fn empty() -> Self {
        Self {
            requests: Vec::new(),
        }
    }

    pub fn new(mut requests: Vec<PersistentSymbolRequest>) -> Result<Self, PersistentSymbolError> {
        requests.sort_by(compare_requests);
        if let Some(duplicate) = requests.windows(2).find(|pair| pair[0].key == pair[1].key) {
            return Err(PersistentSymbolError::DuplicateRequest(duplicate[0].key));
        }
        Ok(Self { requests })
    }

    pub fn requests(&self) -> &[PersistentSymbolRequest] {
        &self.requests
    }

    pub fn into_requests(self) -> Vec<PersistentSymbolRequest> {
        self.requests
    }
}

impl WireEncode for PersistentSymbolRequestTable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.requests.len() as u64)?;
        for request in &self.requests {
            request.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistentSymbolError {
    LinkageNotAllowed {
        kind: PersistentSymbolKind,
        linkage: LinkageClass,
    },
    DuplicateRequest(PersistentSymbolKey),
}

impl fmt::Display for PersistentSymbolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LinkageNotAllowed { kind, linkage } => write!(
                formatter,
                "linkage {linkage:?} is not allowed for persistent symbol kind {}",
                kind.symbol_tag()
            ),
            Self::DuplicateRequest(key) => write!(
                formatter,
                "duplicate persistent symbol request for {}",
                MangledSymbol::from_key(key)
            ),
        }
    }
}

fn compare_requests(
    left: &PersistentSymbolRequest,
    right: &PersistentSymbolRequest,
) -> std::cmp::Ordering {
    left.key
        .kind()
        .tag()
        .cmp(&right.key.kind().tag())
        .then_with(|| left.key.owner_bytes().cmp(right.key.owner_bytes()))
}

impl std::error::Error for PersistentSymbolError {}

fn linkage_allowed_by_kind(kind: PersistentSymbolKind, linkage: LinkageClass) -> bool {
    if linkage == LinkageClass::RuntimeAbi {
        return false;
    }
    match kind {
        PersistentSymbolKind::OdrMember => linkage == LinkageClass::OdrWeak,
        PersistentSymbolKind::ImageDescriptor | PersistentSymbolKind::GeneratedBridge => {
            linkage == LinkageClass::ConeStrong
        }
        PersistentSymbolKind::DispatchSlot => matches!(
            linkage,
            LinkageClass::ConeStrong | LinkageClass::TemplateSupportHidden
        ),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        LinkageClass, MangledSymbol, ManglingSchemaIdentity, PersistentSymbolError,
        PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    };
    use crate::{
        ConeIdentity, GeneratedBridgeAtomId, ObjectDefinitionAtomId, OdrMemberId,
        PersistentCallableBodyId, PersistentDispatchSlotId, PersistentDispatchTableId,
        PersistentExactTypeId, PersistentImmortalObjectId, PersistentInitializationUnitId,
        PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
    };

    #[test]
    fn every_symbol_kind_has_the_frozen_tag_and_spelling() {
        let bytes = ConeIdentity::CORE.0;
        let keys = [
            PersistentSymbolKey::CallableBody(PersistentCallableBodyId(bytes)),
            PersistentSymbolKey::StaticStorage(PersistentStaticStorageId(bytes)),
            PersistentSymbolKey::ImmortalObject(PersistentImmortalObjectId(bytes)),
            PersistentSymbolKey::TypeDescriptor(PersistentExactTypeId(bytes)),
            PersistentSymbolKey::Layout(PersistentLayoutId(bytes)),
            PersistentSymbolKey::ScanProgram(PersistentScanId(bytes)),
            PersistentSymbolKey::DispatchTable(PersistentDispatchTableId(bytes)),
            PersistentSymbolKey::DispatchSlot(PersistentDispatchSlotId(bytes)),
            PersistentSymbolKey::InitializationCell(PersistentInitializationUnitId(bytes)),
            PersistentSymbolKey::InitializationDescriptor(PersistentInitializationUnitId(bytes)),
            PersistentSymbolKey::RootRegistration(PersistentStaticStorageId(bytes)),
            PersistentSymbolKey::ImmortalRegistration(PersistentImmortalObjectId(bytes)),
            PersistentSymbolKey::InitializationRegistration(PersistentInitializationUnitId(bytes)),
            PersistentSymbolKey::TypeRegistration(PersistentExactTypeId(bytes)),
            PersistentSymbolKey::SafepointRegistration(PersistentSafepointSiteId(bytes)),
            PersistentSymbolKey::CallableRegistration(PersistentCallableBodyId(bytes)),
            PersistentSymbolKey::ImageDescriptor(ConeIdentity::CORE),
            PersistentSymbolKey::GeneratedBridge(GeneratedBridgeAtomId(bytes)),
            PersistentSymbolKey::OdrMember(OdrMemberId(bytes)),
            PersistentSymbolKey::DefinitionBoundaryStart(ObjectDefinitionAtomId(bytes)),
            PersistentSymbolKey::DefinitionBoundaryEnd(ObjectDefinitionAtomId(bytes)),
        ];

        for (index, key) in keys.into_iter().enumerate() {
            let expected_tag = index as u64 + 1;
            assert_eq!(key.kind().tag(), expected_tag);
            let encoded = encode(&key).unwrap();
            assert_eq!(&encoded[..4], &[0xa2, 0x00, expected_tag as u8, 0x01]);
            assert_eq!(&encoded[4..6], &[0x58, 0x20]);

            let symbol = MangledSymbol::from_key(&key);
            assert_eq!(symbol.as_str().len(), 75);
            assert_eq!(
                symbol.as_str(),
                format!("scoop$1${}${}", key.kind().symbol_tag(), ConeIdentity::CORE)
            );
        }
    }

    #[test]
    fn request_has_fixed_wire_without_redundant_symbol_text() {
        let key = PersistentSymbolKey::CallableBody(PersistentCallableBodyId(ConeIdentity::CORE.0));
        let request = PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap();

        assert_eq!(
            hex(&encode(&request).unwrap()),
            format!("a201a20001015820{}0201", ConeIdentity::CORE)
        );
        assert_eq!(request.key(), key);
        assert_eq!(request.linkage(), LinkageClass::ConeStrong);
        assert_eq!(request.symbol(), MangledSymbol::from_key(&key));
        assert_eq!(
            hex(&encode(&ManglingSchemaIdentity).unwrap()),
            "6d70657273697374656e742d7631"
        );
    }

    #[test]
    fn linkage_kind_matrix_rejects_impossible_requests() {
        let bytes = ConeIdentity::CORE.0;
        let invalid = [
            (
                PersistentSymbolKey::CallableBody(PersistentCallableBodyId(bytes)),
                LinkageClass::RuntimeAbi,
            ),
            (
                PersistentSymbolKey::OdrMember(OdrMemberId(bytes)),
                LinkageClass::ConeStrong,
            ),
            (
                PersistentSymbolKey::ImageDescriptor(ConeIdentity::CORE),
                LinkageClass::OdrWeak,
            ),
            (
                PersistentSymbolKey::GeneratedBridge(GeneratedBridgeAtomId(bytes)),
                LinkageClass::TemplateSupportHidden,
            ),
            (
                PersistentSymbolKey::DispatchSlot(PersistentDispatchSlotId(bytes)),
                LinkageClass::OdrWeak,
            ),
        ];

        for (key, linkage) in invalid {
            assert_eq!(
                PersistentSymbolRequest::new(key, linkage),
                Err(PersistentSymbolError::LinkageNotAllowed {
                    kind: key.kind(),
                    linkage,
                })
            );
        }
        assert!(
            PersistentSymbolRequest::new(
                PersistentSymbolKey::OdrMember(OdrMemberId(bytes)),
                LinkageClass::OdrWeak,
            )
            .is_ok()
        );
    }

    #[test]
    fn request_table_sorts_by_tag_and_owner_and_rejects_duplicate_keys() {
        let body_key = PersistentSymbolKey::CallableBody(PersistentCallableBodyId(
            ConeIdentity::SINGLE_FILE.0,
        ));
        let storage_key =
            PersistentSymbolKey::StaticStorage(PersistentStaticStorageId(ConeIdentity::CORE.0));
        let body = PersistentSymbolRequest::new(body_key, LinkageClass::ConeStrong).unwrap();
        let storage = PersistentSymbolRequest::new(storage_key, LinkageClass::ConeStrong).unwrap();
        let table = PersistentSymbolRequestTable::new(vec![storage, body]).unwrap();

        assert_eq!(table.requests(), &[body, storage]);
        assert_eq!(table.clone().into_requests(), vec![body, storage]);
        assert_eq!(hex(&encode(&table).unwrap()).chars().next(), Some('8'));

        let conflicting = PersistentSymbolRequest::new(body_key, LinkageClass::OdrWeak).unwrap();
        assert_eq!(
            PersistentSymbolRequestTable::new(vec![body, conflicting]),
            Err(PersistentSymbolError::DuplicateRequest(body_key))
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
