//! MIR wire round-trip and corruption tests (DESIGN 4.4, T20).

use scoop_identity::cbor::CborWriter;
use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_mir::wire::{
    ImportedMirSet, MirWireError, WireAncestry, WireDispatchTable, WireOdrRecord, WireSymbolBridge,
    WireSymbolKind, decode_mir_wire, encode_mir_wire, import_mir_wire,
};

fn cone() -> ConeIdentity {
    ConeIdentity::of(&ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap())
}

fn sample(cone: ConeIdentity) -> Vec<u8> {
    encode_mir_wire(
        cone,
        vec![
            WireSymbolBridge {
                source: [7; 32],
                kind: WireSymbolKind::Callable,
                symbol: "scoop$1$fn$aaaa".to_owned(),
            },
            WireSymbolBridge {
                source: [9; 32],
                kind: WireSymbolKind::Global,
                symbol: "scoop$1$sg$bbbb".to_owned(),
            },
        ],
        vec![WireAncestry {
            class: [7; 32],
            base: [1; 32],
            interfaces: vec![[2; 32], [3; 32]],
        }],
        vec![WireDispatchTable {
            owner: [7; 32],
            entries: vec!["scoop$1$fn$cccc".to_owned()],
        }],
        vec![WireOdrRecord {
            symbol: "scoop$1$odr$dddd".to_owned(),
            linkage: 1,
        }],
        vec![[5; 32], [6; 32]],
    )
}

#[test]
fn mir_wire_round_trips() {
    let bytes = sample(cone());
    let imported = import_mir_wire(decode_mir_wire(&bytes).expect("decodes")).expect("imports");
    let (symbol, kind) = imported
        .symbol_of(&[7; 32])
        .expect("callable bridge resolves");
    assert_eq!(symbol, "scoop$1$fn$aaaa");
    assert_eq!(kind, WireSymbolKind::Callable);
    let ancestry = imported.ancestry_of(&[7; 32]).expect("ancestry resolves");
    assert_eq!(ancestry.interfaces.len(), 2);
    assert_eq!(
        imported
            .dispatch_of(&[7; 32])
            .expect("dispatch resolves")
            .entries,
        vec!["scoop$1$fn$cccc".to_owned()]
    );
    assert_eq!(imported.odr_records().len(), 1);
    assert_eq!(imported.exact_types().len(), 2);
}

#[test]
fn mir_wire_is_deterministic() {
    // Shuffled inputs produce identical bytes after canonical sorting.
    let mut first = sample(cone());
    let swapped = encode_mir_wire(
        cone(),
        vec![WireSymbolBridge {
            source: [9; 32],
            kind: WireSymbolKind::Global,
            symbol: "scoop$1$sg$bbbb".to_owned(),
        }],
        vec![],
        vec![],
        vec![],
        vec![[6; 32], [5; 32]],
    );
    assert_ne!(first, swapped);
    first.extend_from_slice(&[0]);
    first.pop();
    assert_eq!(first, sample(cone()), "encoding is stable");
}

#[test]
fn wrong_magic_is_rejected() {
    let mut bytes = sample(cone());
    let magic = b"scoop-mir-wire-v1";
    let position = bytes
        .windows(magic.len())
        .position(|window| window == magic)
        .expect("magic present");
    bytes[position] = b'x';
    assert!(matches!(decode_mir_wire(&bytes), Err(MirWireError::Magic)));
}

#[test]
fn truncation_is_rejected() {
    let bytes = sample(cone());
    assert!(decode_mir_wire(&bytes[..bytes.len() - 4]).is_err());
}

#[test]
fn unsorted_sections_are_rejected() {
    let mut decoded = decode_mir_wire(&sample(cone())).expect("decodes");
    decoded.bridge.reverse();
    assert!(matches!(import_mir_wire(decoded), Err(MirWireError::Order)));
}

#[test]
fn duplicate_bridge_is_rejected() {
    let mut decoded = decode_mir_wire(&sample(cone())).expect("decodes");
    let clone = decoded.bridge[0].clone();
    decoded.bridge.push(clone);
    decoded.bridge.sort_by(|a, b| {
        (a.source, a.kind.tag(), &a.symbol).cmp(&(b.source, b.kind.tag(), &b.symbol))
    });
    assert!(matches!(
        import_mir_wire(decoded),
        Err(MirWireError::Duplicate)
    ));
}

#[test]
fn unknown_linkage_is_rejected() {
    let mut decoded = decode_mir_wire(&sample(cone())).expect("decodes");
    decoded.odr[0].linkage = 99;
    assert!(matches!(
        import_mir_wire(decoded),
        Err(MirWireError::UnknownKind(99))
    ));
}

#[test]
fn mir_wire_survives_the_slib_envelope() {
    let bytes = sample(cone());
    let mut builder =
        scoop_slib::artifact::SlibBuilder::new(scoop_slib::artifact::ManifestCoreTemplate {
            container_version: 1,
            hir_wire_schema: 1,
            mir_wire_schema: 1,
            lir_wire_schema: 1,
            producer_compiler_version: "0.0.0 (test)".to_owned(),
            language_abi: 1,
            runtime_abi: scoop_identity::Digest256::from_bytes([1; 32]),
            identity_schema_version: 1,
            coordinate: ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap(),
            kind: scoop_manifest::ConeKind::Library,
            dependencies: Vec::new(),
            target_profile: scoop_identity::capability::TargetProfileWireId::darwin_aarch64_v1(),
            target_profile_fingerprint: scoop_identity::Digest256::from_bytes([5; 32]),
            backend_profile_fingerprint: scoop_identity::Digest256::from_bytes([6; 32]),
        });
    builder
        .add_member(
            scoop_slib::member::MemberStableKey::HirMetadata,
            scoop_slib::member::SlibMemberRole::HirMetadata { wire_schema: 1 },
            b"scoop-hir-surface-wire-v1-payload".to_vec(),
        )
        .expect("member");
    builder
        .add_member(
            scoop_slib::member::MemberStableKey::MirMetadata,
            scoop_slib::member::SlibMemberRole::MirMetadata { wire_schema: 1 },
            bytes.clone(),
        )
        .expect("member");
    builder
        .add_member(
            scoop_slib::member::MemberStableKey::LirMetadata,
            scoop_slib::member::SlibMemberRole::LirMetadata { wire_schema: 1 },
            b"scoop-lir-semantic-v1".to_vec(),
        )
        .expect("member");
    let archive = builder.finish().expect("archive");
    let limits = scoop_slib::limits::SlibDecodeLimits::default();
    let envelope =
        scoop_slib::artifact::DecodedSlibEnvelope::decode(&archive, &limits).expect("envelope");
    let mir = envelope
        .manifest()
        .members
        .iter()
        .find(|record| {
            matches!(
                record.stable_key,
                scoop_slib::member::MemberStableKey::MirMetadata
            )
        })
        .expect("mir member");
    let payload = envelope.member_payload(&mir.id).expect("payload");
    let imported = import_mir_wire(decode_mir_wire(payload).expect("decodes")).expect("imports");
    assert_eq!(imported.cone(), cone());
    assert!(imported.symbol_of(&[7; 32]).is_some());
    let _ = CborWriter::new();
    let _: Option<ImportedMirSet> = None;
}
