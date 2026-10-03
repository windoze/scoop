use super::*;

mod imported_handles;

#[test]
fn native_boundary_producer_uses_shared_core_metadata_for_a_narrow_integer() {
    source_dispatch::with_hir_source(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-native-boundary/single.scoop"
        )),
        |output, core| {
            let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
            let integer = protocols
                .protocols()
                .fundamental_types()
                .integer(hir::IntegerKind::SIGNED_8)
                .persistent();
            let records = output.output().native_boundary_types.records();
            assert_eq!(records.len(), 1);
            assert_eq!(
                records[0].owner(),
                hir::NativeBoundaryNominalOwner::Concrete(integer)
            );
            assert!(
                matches!(records[0].shape(), hir::NativeBoundaryNominalShape::Intrinsic(representation) if representation.family() == hir::IntrinsicTypeKind::Integer(hir::IntegerKind::SIGNED_8))
            );
        },
    );
}

#[test]
fn native_boundary_producer_closes_local_struct_enum_and_all_imported_primitives() {
    source_dispatch::with_hir_source(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-native-boundary/combined.scoop"
        )),
        |output, core| {
            let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
            let fundamental = protocols.protocols().fundamental_types();
            let mut names = BTreeMap::new();
            for (kind, name) in hir::IntegerKind::ALL.into_iter().zip([
                "Int8", "Int16", "Int", "Long", "UInt8", "UInt16", "UInt", "ULong",
            ]) {
                names.insert(
                    hir::NativeBoundaryNominalOwner::Concrete(
                        fundamental.integer(kind).persistent(),
                    ),
                    name.to_owned(),
                );
            }
            for (id, name) in [
                (fundamental.boolean().persistent(), "Boolean"),
                (fundamental.string().persistent(), "String"),
                (fundamental.unit().persistent(), "Unit"),
            ] {
                names.insert(
                    hir::NativeBoundaryNominalOwner::Concrete(id),
                    name.to_owned(),
                );
            }
            let export = output.output().export.module();
            for (id, d) in export.structs.iter() {
                names.insert(
                    hir::NativeBoundaryNominalOwner::Concrete(
                        export.nominal_identities[id].concrete_type_id().unwrap(),
                    ),
                    d.name.clone(),
                );
            }
            for (id, d) in export.enums.iter() {
                names.insert(
                    hir::NativeBoundaryNominalOwner::Concrete(
                        export.nominal_identities[id].concrete_type_id().unwrap(),
                    ),
                    d.name.clone(),
                );
            }
            let records = output.output().native_boundary_types.records();
            assert_eq!(records.len(), 14);
            for record in records {
                assert_eq!(
                    record.c_abi(),
                    hir::NativeBoundaryCAbiV1::SourceRepresentation
                );
                let name = names[&record.owner()].as_str();
                match record.shape() {
                    hir::NativeBoundaryNominalShape::Intrinsic(representation) => {
                        let family = representation.family();
                        assert_eq!(family.source_name(), name);
                        assert!(matches!(
                            family,
                            hir::IntrinsicTypeKind::Integer(_)
                                | hir::IntrinsicTypeKind::Boolean
                                | hir::IntrinsicTypeKind::String
                        ));
                    }
                    hir::NativeBoundaryNominalShape::Struct { c_layout, fields } => {
                        let count = match name {
                            "Packed" => {
                                assert_eq!(
                                    *c_layout,
                                    hir::NativeBoundaryCLayoutPolicy::CLayout {
                                        aligned: scoop_identity::CLayoutOverride::Bytes(
                                            scoop_identity::CLayoutByteAlignment::Bytes8
                                        ),
                                        packed: scoop_identity::CLayoutOverride::Bytes(
                                            scoop_identity::CLayoutByteAlignment::Bytes1
                                        ),
                                    }
                                );
                                2
                            }
                            "Unit" => 0,
                            "Wrapper" => 1,
                            other => panic!("unexpected native struct {other}"),
                        };
                        assert_eq!(fields.len(), count);
                        if name != "Packed" {
                            assert_eq!(*c_layout, hir::NativeBoundaryCLayoutPolicy::NotCLayout);
                        }
                    }
                    hir::NativeBoundaryNominalShape::Enum { variants } => {
                        assert_eq!(name, "Choice");
                        assert_eq!(
                            variants
                                .iter()
                                .map(|v| v.fields().len())
                                .collect::<Vec<_>>(),
                            [1, 1, 0]
                        );
                    }
                    hir::NativeBoundaryNominalShape::Reference => {
                        panic!("the native closure has no opaque references");
                    }
                }
                assert_ne!(name, "Unused");
            }
        },
    );
}
