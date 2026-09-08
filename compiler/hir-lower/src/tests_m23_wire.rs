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
    assert!(matches!(
        error,
        Err(scoop_hir::wire::HirWireError::Truncated | scoop_hir::wire::HirWireError::TrailingBytes)
    ));
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
