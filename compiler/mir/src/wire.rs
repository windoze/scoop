//! MIR wire schema (DESIGN 4.4, T20 batch 1). The document carries the
//! relations downstream MIR cannot rebuild from names: the HIR
//! persistent-id to external symbol bridge, exact ancestry, dispatch
//! tables, emitted specialization ODR/linkage records and the
//! materialized exact-type identities. Canonical CBOR; every list is
//! sorted so the encoding is deterministic.

use scoop_identity::ConeIdentity;
use scoop_identity::cbor::{CborReader, CborWriter};

/// Wire schema domain tag.
pub const MIR_WIRE_MAGIC: &str = "scoop-mir-wire-v1";

/// The closed symbol-bridge kind set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WireSymbolKind {
    Callable,
    Global,
    Constructor,
    Accessor,
    Specialization,
}

impl WireSymbolKind {
    pub fn tag(self) -> u64 {
        match self {
            Self::Callable => 1,
            Self::Global => 2,
            Self::Constructor => 3,
            Self::Accessor => 4,
            Self::Specialization => 5,
        }
    }

    fn from_tag(tag: u64) -> Option<Self> {
        Some(match tag {
            1 => Self::Callable,
            2 => Self::Global,
            3 => Self::Constructor,
            4 => Self::Accessor,
            5 => Self::Specialization,
            _ => return None,
        })
    }
}

/// One bridge entry: the persistent HIR source identity and its
/// external link symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireSymbolBridge {
    pub source: [u8; 32],
    pub kind: WireSymbolKind,
    pub symbol: String,
}

/// Exact ancestry of one class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireAncestry {
    pub class: [u8; 32],
    /// Empty bytes mean no base class.
    pub base: [u8; 32],
    pub interfaces: Vec<[u8; 32]>,
}

/// One dispatch table: the owning callable's persistent identity and
/// its slot targets' symbols, in slot order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireDispatchTable {
    pub owner: [u8; 32],
    pub entries: Vec<String>,
}

/// One emitted specialization's ODR/linkage record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireOdrRecord {
    pub symbol: String,
    /// 1 = linkonce_odr.
    pub linkage: u64,
}

/// The wire document before commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedMirWire {
    pub cone: ConeIdentity,
    pub bridge: Vec<WireSymbolBridge>,
    pub ancestry: Vec<WireAncestry>,
    pub dispatch: Vec<WireDispatchTable>,
    pub odr: Vec<WireOdrRecord>,
    pub exact_types: Vec<[u8; 32]>,
}

/// A structured MIR wire failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirWireError {
    Magic,
    Truncated,
    TrailingBytes,
    Malformed(&'static str),
    UnknownKind(u64),
    Order,
    Duplicate,
    CountExceeded(&'static str),
}

/// The committed consumer view (DESIGN `ImportedMirSet`).
#[derive(Debug, Clone)]
pub struct ImportedMirSet {
    cone: ConeIdentity,
    bridge: Vec<WireSymbolBridge>,
    ancestry: Vec<WireAncestry>,
    dispatch: Vec<WireDispatchTable>,
    odr: Vec<WireOdrRecord>,
    exact_types: Vec<[u8; 32]>,
}

impl ImportedMirSet {
    pub fn cone(&self) -> ConeIdentity {
        self.cone
    }

    /// The external symbol of one persistent HIR source identity.
    pub fn symbol_of(&self, source: &[u8; 32]) -> Option<(&str, WireSymbolKind)> {
        self.bridge
            .iter()
            .find(|entry| &entry.source == source)
            .map(|entry| (entry.symbol.as_str(), entry.kind))
    }

    pub fn ancestry_of(&self, class: &[u8; 32]) -> Option<&WireAncestry> {
        self.ancestry.iter().find(|entry| &entry.class == class)
    }

    pub fn dispatch_of(&self, owner: &[u8; 32]) -> Option<&WireDispatchTable> {
        self.dispatch.iter().find(|entry| &entry.owner == owner)
    }

    pub fn odr_records(&self) -> &[WireOdrRecord] {
        &self.odr
    }

    pub fn exact_types(&self) -> &[[u8; 32]] {
        &self.exact_types
    }

    pub fn bridge(&self) -> &[WireSymbolBridge] {
        &self.bridge
    }
}

/// Encodes one document. Callers hand over pre-sorted sections; the
/// encoder re-sorts canonically so equal inputs produce equal bytes.
pub fn encode_mir_wire(
    cone: ConeIdentity,
    mut bridge: Vec<WireSymbolBridge>,
    mut ancestry: Vec<WireAncestry>,
    mut dispatch: Vec<WireDispatchTable>,
    mut odr: Vec<WireOdrRecord>,
    mut exact_types: Vec<[u8; 32]>,
) -> Vec<u8> {
    bridge.sort_by(|a, b| {
        (a.source, a.kind.tag(), &a.symbol).cmp(&(b.source, b.kind.tag(), &b.symbol))
    });
    ancestry.sort_by_key(|entry| entry.class);
    dispatch.sort_by_key(|table| table.owner);
    odr.sort_by_key(|record| (record.symbol.clone(), record.linkage));
    exact_types.sort();
    let mut writer = CborWriter::new();
    writer.map(7);
    writer.field(1).text(MIR_WIRE_MAGIC);
    writer.field(2).bytes(cone.as_bytes());
    writer.field(3);
    writer.array(bridge.len() as u64);
    for entry in &bridge {
        writer.map(3);
        writer.field(1).bytes(&entry.source);
        writer.field(2).unsigned(entry.kind.tag());
        writer.field(3).text(&entry.symbol);
    }
    writer.field(4);
    writer.array(ancestry.len() as u64);
    for entry in &ancestry {
        writer.map(3);
        writer.field(1).bytes(&entry.class);
        writer.field(2).bytes(&entry.base);
        writer.field(3);
        writer.array(entry.interfaces.len() as u64);
        for interface in &entry.interfaces {
            writer.bytes(interface);
        }
    }
    writer.field(5);
    writer.array(dispatch.len() as u64);
    for table in &dispatch {
        writer.map(2);
        writer.field(1).bytes(&table.owner);
        writer.field(2);
        writer.array(table.entries.len() as u64);
        for entry in &table.entries {
            writer.text(entry);
        }
    }
    writer.field(6);
    writer.array(odr.len() as u64);
    for record in &odr {
        writer.map(2);
        writer.field(1).text(&record.symbol);
        writer.field(2).unsigned(record.linkage);
    }
    writer.field(7);
    writer.array(exact_types.len() as u64);
    for exact in &exact_types {
        writer.bytes(exact);
    }
    writer.into_bytes()
}

fn wire_error(error: scoop_identity::CborError) -> MirWireError {
    match error {
        scoop_identity::CborError::UnexpectedEof => MirWireError::Truncated,
        scoop_identity::CborError::TrailingBytes(_) => MirWireError::TrailingBytes,
        _ => MirWireError::Malformed("malformed canonical CBOR"),
    }
}

/// Wire decode under the 4.5 budgets.
pub fn decode_mir_wire(data: &[u8]) -> Result<DecodedMirWire, MirWireError> {
    let mut reader = CborReader::new(data, 128);
    let mut cone = [0u8; 32];
    let mut bridge = Vec::new();
    let mut ancestry = Vec::new();
    let mut dispatch = Vec::new();
    let mut odr = Vec::new();
    let mut exact_types = Vec::new();
    let mut saw_magic = false;
    {
        let mut map = reader.map().map_err(wire_error)?;
        while let Some(key) = map.next_key().map_err(wire_error)? {
            match key {
                1 => {
                    let text = map.text().map_err(wire_error)?;
                    if text != MIR_WIRE_MAGIC {
                        return Err(MirWireError::Magic);
                    }
                    saw_magic = true;
                }
                2 => {
                    let bytes = map.bytes().map_err(wire_error)?;
                    cone = bytes
                        .try_into()
                        .map_err(|_| MirWireError::Malformed("cone must be 32 bytes"))?;
                }
                3 => {
                    let mut seq = map.array().map_err(wire_error)?;
                    if seq.count() > 16_777_216 {
                        return Err(MirWireError::CountExceeded("bridge"));
                    }
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(wire_error)?;
                        let mut source = [0u8; 32];
                        let mut kind = None;
                        let mut symbol = String::new();
                        while let Some(field) = record.next_key().map_err(wire_error)? {
                            match field {
                                1 => {
                                    let bytes = record.bytes().map_err(wire_error)?;
                                    source = bytes.try_into().map_err(|_| {
                                        MirWireError::Malformed("source must be 32 bytes")
                                    })?;
                                }
                                2 => {
                                    let tag = record.unsigned().map_err(wire_error)?;
                                    kind = Some(
                                        WireSymbolKind::from_tag(tag)
                                            .ok_or(MirWireError::UnknownKind(tag))?,
                                    );
                                }
                                3 => {
                                    symbol = record.text().map_err(wire_error)?.to_owned();
                                }
                                _ => return Err(MirWireError::Malformed("unknown bridge field")),
                            }
                        }
                        bridge.push(WireSymbolBridge {
                            source,
                            kind: kind.ok_or(MirWireError::Malformed("bridge without kind"))?,
                            symbol,
                        });
                    }
                }
                4 => {
                    let mut seq = map.array().map_err(wire_error)?;
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(wire_error)?;
                        let mut class = [0u8; 32];
                        let mut base = [0u8; 32];
                        let mut interfaces = Vec::new();
                        while let Some(field) = record.next_key().map_err(wire_error)? {
                            match field {
                                1 => {
                                    let bytes = record.bytes().map_err(wire_error)?;
                                    class = bytes.try_into().map_err(|_| {
                                        MirWireError::Malformed("class must be 32 bytes")
                                    })?;
                                }
                                2 => {
                                    let bytes = record.bytes().map_err(wire_error)?;
                                    base = bytes.try_into().map_err(|_| {
                                        MirWireError::Malformed("base must be 32 bytes")
                                    })?;
                                }
                                3 => {
                                    let mut entries = record.array().map_err(wire_error)?;
                                    for _ in 0..entries.count() {
                                        let bytes = entries.bytes().map_err(wire_error)?;
                                        interfaces.push(bytes.try_into().map_err(|_| {
                                            MirWireError::Malformed("interface must be 32 bytes")
                                        })?);
                                    }
                                }
                                _ => return Err(MirWireError::Malformed("unknown ancestry field")),
                            }
                        }
                        ancestry.push(WireAncestry {
                            class,
                            base,
                            interfaces,
                        });
                    }
                }
                5 => {
                    let mut seq = map.array().map_err(wire_error)?;
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(wire_error)?;
                        let mut owner = [0u8; 32];
                        let mut entries = Vec::new();
                        while let Some(field) = record.next_key().map_err(wire_error)? {
                            match field {
                                1 => {
                                    let bytes = record.bytes().map_err(wire_error)?;
                                    owner = bytes.try_into().map_err(|_| {
                                        MirWireError::Malformed("owner must be 32 bytes")
                                    })?;
                                }
                                2 => {
                                    let mut list = record.array().map_err(wire_error)?;
                                    for _ in 0..list.count() {
                                        entries.push(list.text().map_err(wire_error)?.to_owned());
                                    }
                                }
                                _ => return Err(MirWireError::Malformed("unknown dispatch field")),
                            }
                        }
                        dispatch.push(WireDispatchTable { owner, entries });
                    }
                }
                6 => {
                    let mut seq = map.array().map_err(wire_error)?;
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(wire_error)?;
                        let mut symbol = String::new();
                        let mut linkage = 0u64;
                        while let Some(field) = record.next_key().map_err(wire_error)? {
                            match field {
                                1 => {
                                    symbol = record.text().map_err(wire_error)?.to_owned();
                                }
                                2 => {
                                    linkage = record.unsigned().map_err(wire_error)?;
                                }
                                _ => return Err(MirWireError::Malformed("unknown odr field")),
                            }
                        }
                        odr.push(WireOdrRecord { symbol, linkage });
                    }
                }
                7 => {
                    let mut seq = map.array().map_err(wire_error)?;
                    for _ in 0..seq.count() {
                        let bytes = seq.bytes().map_err(wire_error)?;
                        exact_types.push(
                            bytes
                                .try_into()
                                .map_err(|_| MirWireError::Malformed("exact must be 32 bytes"))?,
                        );
                    }
                }
                _ => return Err(MirWireError::Malformed("unknown document field")),
            }
        }
    }
    if !saw_magic {
        return Err(MirWireError::Magic);
    }
    reader.finish().map_err(wire_error)?;
    Ok(DecodedMirWire {
        cone: ConeIdentity::from_bytes(&cone),
        bridge,
        ancestry,
        dispatch,
        odr,
        exact_types,
    })
}

/// Structural validation and commit: canonical section order, closed
/// linkage tags, no duplicate bridge identities.
pub fn import_mir_wire(decoded: DecodedMirWire) -> Result<ImportedMirSet, MirWireError> {
    for pair in decoded.bridge.windows(2) {
        match (pair[0].source, pair[0].kind.tag(), &pair[0].symbol).cmp(&(
            pair[1].source,
            pair[1].kind.tag(),
            &pair[1].symbol,
        )) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => return Err(MirWireError::Duplicate),
            std::cmp::Ordering::Greater => return Err(MirWireError::Order),
        }
    }
    for pair in decoded.ancestry.windows(2) {
        if pair[0].class >= pair[1].class {
            return Err(MirWireError::Order);
        }
    }
    for pair in decoded.dispatch.windows(2) {
        if pair[0].owner >= pair[1].owner {
            return Err(MirWireError::Order);
        }
    }
    for record in &decoded.odr {
        if record.linkage != 1 {
            return Err(MirWireError::UnknownKind(record.linkage));
        }
    }
    for pair in decoded.exact_types.windows(2) {
        if pair[0] >= pair[1] {
            return Err(MirWireError::Order);
        }
    }
    Ok(ImportedMirSet {
        cone: decoded.cone,
        bridge: decoded.bridge,
        ancestry: decoded.ancestry,
        dispatch: decoded.dispatch,
        odr: decoded.odr,
        exact_types: decoded.exact_types,
    })
}
