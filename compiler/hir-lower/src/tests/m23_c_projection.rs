use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-native-boundary")
            .join(name),
    )
    .unwrap()
}

#[test]
fn c_handle_source_fixtures_preserve_native_hir_and_mir_projections() {
    for (name, handles) in [
        ("handle-c.scoop", &["GcHandle"][..]),
        ("handle-c-combined.scoop", &["GcHandle", "PinnedPtr"][..]),
    ] {
        let text = fixture(name);
        let output = lower(&[complete_core_file(), scoop_parser::parse(&text).unwrap()]).unwrap();
        let mir = scoop_mir_lower::lower(&output.local).unwrap();
        scoop_mir::validate_module(&mir).unwrap();
        for name in handles {
            let (id, _) = output
                .export
                .structs
                .iter()
                .find(|(_, definition)| definition.name == *name)
                .unwrap();
            let origin = output.export.nominal_identities[id]
                .generic_type_id()
                .unwrap();
            let native = output
                .native_boundary_types
                .records()
                .iter()
                .find(|record| {
                    record.owner() == hir::NativeBoundaryNominalOwner::GenericTemplate(origin)
                })
                .unwrap();
            let hir::NativeBoundaryCAbiV1::UInt64Field { field } = native.c_abi() else {
                panic!("handle must retain its declared C projection");
            };
            let local = output
                .local
                .module()
                .structs
                .iter()
                .find_map(|(_, definition)| match &definition.representation {
                    hir::concrete::StructRepresentation::Declared {
                        c_abi: hir::concrete::StructCAbi::UInt64Field { field: actual },
                        fields,
                        ..
                    } if *actual == field => {
                        assert_eq!(fields.len(), 1);
                        assert_eq!(fields[0].identity, field);
                        assert!(definition.gc_free);
                        Some(definition)
                    }
                    _ => None,
                })
                .unwrap();
            let lowered = mir
                .structs
                .iter()
                .find_map(|(_, definition)| match &definition.representation {
                    scoop_mir::StructRepresentation::Declared {
                        c_abi: scoop_mir::StructCAbi::UInt64Field { field: actual },
                        fields,
                        ..
                    } if *actual == field => {
                        assert_eq!(fields[0].identity, field);
                        assert_eq!(
                            fields[0].ty,
                            scoop_mir::Type::Integer(scoop_mir::IntegerKind::UNSIGNED_64)
                        );
                        assert!(definition.gc_free);
                        Some(definition)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(local.name, lowered.name);
        }
        if handles.len() == 2 {
            let option = defined_export_core(&output.export).option.enumeration();
            let origin = output.export.nominal_identities[option]
                .generic_type_id()
                .unwrap();
            assert!(
                output
                    .native_boundary_types
                    .records()
                    .iter()
                    .any(|record| record.owner()
                        == hir::NativeBoundaryNominalOwner::GenericTemplate(origin)
                        && matches!(
                            record.c_abi(),
                            hir::NativeBoundaryCAbiV1::NullablePointer { .. }
                        ))
            );
            assert_eq!(mir.callback_bridges.len(), 1);
        }
    }
}
