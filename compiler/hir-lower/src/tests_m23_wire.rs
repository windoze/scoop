//! HIR surface wire round-trip and corruption tests (DESIGN 4.3/4.5).

use super::*;
use crate::tests::core::core_file;

fn parse(text: &str) -> ast::SourceFile {
    scoop_parser::parse(text).expect("test source parses")
}

const LIBRARY: &str = r#"package org.lib

public struct Plain(val value: Int)

public struct Box<T>(val item: T)

public fun <T> identity(value: T): T {
    return value
}

public fun visible(value: Int): Int {
    return value
}

internal fun hidden(): Int {
    return 0
}

public typealias Name = String

public open class Base public constructor() {
    protected fun shield(): Int {
        return 1
    }
}
"#;

fn encoded_library() -> Vec<u8> {
    let user = "fun main() {}\n";
    let output = lower(&[core_file(), parse(LIBRARY), parse(user)]).expect("lowers");
    let world = scoop_hir::PersistentWorld::single_unit(
        crate::test_cone_identity(),
        crate::test_cone_identity(),
    );
    let mut ids = scoop_hir::PersistentIds::new(&output.export, &world);
    scoop_hir::wire::encode_surface_wire(&mut ids, &output.export, &output.export.export_surfaces)
        .expect("encodes")
}

#[test]
fn surface_wire_round_trips() {
    let bytes = encoded_library();
    let decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    let imported = scoop_hir::wire::import_surface_wire(decoded).expect("imports");
    // Every definition carries a purpose bit and a name.
    assert!(!imported.definitions().is_empty());
    for definition in imported.definitions() {
        assert!(
            !definition.name.is_empty()
                || definition.kind == scoop_hir::wire::WireEntityKind::ObjectType,
            "definitions carry names"
        );
        assert_ne!(definition.purposes.0, 0, "definitions carry purposes");
    }
    // The public lookup surface survives the trip.
    assert!(imported.public_lookup().len() >= 6);
    // The value binding for `visible` resolves to a Function root.
    let roots = imported
        .binding_roots(&["org".to_owned(), "lib".to_owned()], "visible", 1)
        .expect("visible binding");
    assert_eq!(roots.len(), 1);
    assert_eq!(
        imported.definition(roots[0]).kind,
        scoop_hir::wire::WireEntityKind::Function
    );
    // The internal function never reached the wire.
    assert!(
        imported
            .binding_roots(&["org".to_owned(), "lib".to_owned()], "hidden", 1)
            .is_none()
    );
}

#[test]
fn truncated_wire_is_rejected() {
    let bytes = encoded_library();
    let error = scoop_hir::wire::decode_surface_wire(&bytes[..bytes.len() - 3]);
    // Truncation surfaces as a structural CBOR failure; every variant
    // is a rejection, never a panic or a partial accept.
    assert!(error.is_err());
}

#[test]
fn wrong_magic_is_rejected() {
    let mut bytes = encoded_library();
    // The magic is the first text field; flip one of its bytes.
    let magic = b"scoop-hir-surface-wire-v1";
    if let Some(position) = bytes
        .windows(magic.len())
        .position(|window| window == magic)
    {
        bytes[position] = b'x';
    }
    assert!(matches!(
        scoop_hir::wire::decode_surface_wire(&bytes),
        Err(scoop_hir::wire::HirWireError::Magic)
    ));
}

#[test]
fn re_encoded_unsorted_definitions_are_rejected() {
    // The canonical order check lives in decode; a document whose
    // definitions descend is rejected when re-read. Build one by
    // swapping the first two entries of a fresh encode via the
    // structural validator exported for tests.
    let bytes = encoded_library();
    let mut decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    assert!(decoded.definitions.len() >= 2);
    decoded.definitions.swap(0, 1);
    assert!(matches!(
        scoop_hir::wire::validate_definition_order(&decoded.definitions),
        Err(scoop_hir::wire::HirWireError::DefinitionOrder)
    ));
}

#[test]
fn purpose_mask_overflow_is_rejected() {
    let bytes = encoded_library();
    let mut decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    decoded.definitions[0].purposes.0 = 0x20;
    assert!(matches!(
        scoop_hir::wire::validate_purpose_masks(&decoded.definitions),
        Err(scoop_hir::wire::HirWireError::PurposeMask(0x20))
    ));
}

#[test]
fn empty_binding_roots_are_rejected() {
    let bytes = encoded_library();
    let mut decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    decoded.bindings.push(scoop_hir::wire::WireBinding {
        package: vec!["x".to_owned()],
        name: "broken".to_owned(),
        namespace: 1,
        roots: Vec::new(),
    });
    assert!(matches!(
        scoop_hir::wire::import_surface_wire(decoded),
        Err(scoop_hir::wire::HirWireError::EmptyRoots)
    ));
}

#[test]
fn out_of_range_root_index_is_rejected() {
    let bytes = encoded_library();
    let mut decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    decoded.bindings.push(scoop_hir::wire::WireBinding {
        package: vec!["x".to_owned()],
        name: "broken".to_owned(),
        namespace: 2,
        roots: vec![u32::MAX],
    });
    assert!(matches!(
        scoop_hir::wire::import_surface_wire(decoded),
        Err(scoop_hir::wire::HirWireError::RootIndex(u32::MAX))
    ));
}

#[test]
fn signatures_and_predicates_round_trip() {
    let bytes = encoded_library();
    let decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    let imported = scoop_hir::wire::import_surface_wire(decoded).expect("imports");
    // Every signature references typed entries; at least one parameter
    // type is the canonical Integer tag.
    assert!(!imported.signatures().is_empty());
    assert!(
        imported
            .types()
            .iter()
            .any(|entry| matches!(entry, scoop_hir::wire::WireTypeEntry::Integer(_)))
    );
    // `identity<T>`'s template is on the template surface and carries a
    // predicate record.
    let templates = imported.template_support();
    let generic_named_identity = templates
        .iter()
        .any(|id| imported.definition(*id).name == "identity");
    assert!(generic_named_identity, "identity template survives");
    // The type table is topologically ordered by construction: every
    // reference points at an earlier entry (validated at import).
    let count = imported.types().len() as u32;
    assert!(count > 0);
}

#[test]
fn corrupt_type_graph_is_rejected() {
    let bytes = encoded_library();
    let mut decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    // A self-referential entry violates the topological invariant.
    if let Some(entry) = decoded.types.first_mut() {
        if let scoop_hir::wire::WireTypeEntry::Ptr(index) = entry {
            *index = 0;
        } else {
            decoded
                .types
                .insert(0, scoop_hir::wire::WireTypeEntry::Ptr(0));
        }
    }
    assert!(scoop_hir::wire::import_surface_wire(decoded).is_err());
}

#[test]
fn surface_wire_survives_the_slib_envelope() {
    let bytes = encoded_library();
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
            coordinate: scoop_identity::ConeCoordinate::new("dev.example", "app", "0.1.0")
                .expect("canonical"),
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
            bytes.clone(),
        )
        .expect("member added");
    // The envelope requires exactly one member per metadata role; the
    // MIR/LIR wire sections are T20/T21, so they carry their domain
    // tags only.
    builder
        .add_member(
            scoop_slib::member::MemberStableKey::MirMetadata,
            scoop_slib::member::SlibMemberRole::MirMetadata { wire_schema: 1 },
            b"scoop-mir-semantic-v1".to_vec(),
        )
        .expect("member added");
    builder
        .add_member(
            scoop_slib::member::MemberStableKey::LirMetadata,
            scoop_slib::member::SlibMemberRole::LirMetadata { wire_schema: 1 },
            b"scoop-lir-semantic-v1".to_vec(),
        )
        .expect("member added");
    let archive = builder.finish().expect("archive built");
    let limits = scoop_slib::limits::SlibDecodeLimits::default();
    let envelope = scoop_slib::artifact::DecodedSlibEnvelope::decode(&archive, &limits)
        .expect("envelope decodes");
    // Locate the HirMetadata member and run the wire reader over it.
    let hir = envelope
        .manifest()
        .members
        .iter()
        .find(|record| {
            matches!(
                record.stable_key,
                scoop_slib::member::MemberStableKey::HirMetadata
            )
        })
        .expect("hir member in directory");
    let payload = envelope.member_payload(&hir.id).expect("payload");
    assert_eq!(payload, &bytes[..], "payload bytes survive the envelope");
    let imported = scoop_hir::wire::import_surface_wire(
        scoop_hir::wire::decode_surface_wire(payload).expect("decodes"),
    )
    .expect("imports");
    assert!(!imported.definitions().is_empty());
}

#[test]
fn generic_template_bodies_round_trip() {
    let bytes = encoded_library();
    let decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    let imported = scoop_hir::wire::import_surface_wire(decoded).expect("imports");
    assert!(!imported.bodies().is_empty(), "template bodies survive");
    // Every body is anchored at a definition that exists and carries
    // the template-support purpose.
    for body in imported.bodies() {
        let definition = imported
            .definitions()
            .get(body.definition as usize)
            .expect("body definition in range");
        assert_ne!(
            definition.purposes.0 & scoop_hir::wire::WirePurposes::TEMPLATE_SUPPORT,
            0,
            "bodies anchor at template-support definitions"
        );
    }
    // Statements decoded structurally (kernel tags all closed).
    let statements: usize = imported
        .bodies()
        .iter()
        .map(|body| body.statements.len())
        .sum();
    assert!(statements > 0);
}

#[test]
fn corrupt_body_is_rejected() {
    let bytes = encoded_library();
    let mut decoded = scoop_hir::wire::decode_surface_wire(&bytes).expect("decodes");
    decoded.bodies.push(scoop_hir::wire::WireBody {
        definition: u32::MAX,
        statements: Vec::new(),
    });
    assert!(matches!(
        scoop_hir::wire::import_surface_wire(decoded),
        Err(scoop_hir::wire::HirWireError::RootIndex(u32::MAX))
    ));
}
