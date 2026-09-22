use super::*;

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
            let mut dump = records
                .iter()
                .map(|record| match record.shape() {
                    hir::NativeBoundaryNominalShape::Intrinsic(representation) => format!(
                        "{} intrinsic {}\n",
                        names[&record.owner()],
                        representation.family().name()
                    ),
                    hir::NativeBoundaryNominalShape::Struct { c_layout, fields } => format!(
                        "{} struct {c_layout:?} fields={}\n",
                        names[&record.owner()],
                        fields.len()
                    ),
                    hir::NativeBoundaryNominalShape::Enum { variants } => format!(
                        "{} enum fields={:?}\n",
                        names[&record.owner()],
                        variants
                            .iter()
                            .map(|v| v.fields().len())
                            .collect::<Vec<_>>()
                    ),
                    hir::NativeBoundaryNominalShape::Reference => {
                        format!("{} reference\n", names[&record.owner()])
                    }
                })
                .collect::<Vec<_>>();
            dump.sort();
            assert_eq!(
                dump.concat(),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-native-boundary/combined.snap"
                ))
            );
            assert!(!dump.concat().contains("Unused"));
        },
    );
}
