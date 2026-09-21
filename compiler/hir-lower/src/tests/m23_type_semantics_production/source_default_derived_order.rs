use super::*;
use source_dispatch::with_source;

#[test]
fn derived_equality_is_available_while_source_defaults_are_prepared() {
    for (source, names) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/derived-default-order.scoop"
            )),
            &["structDefault", "enumDefault"][..],
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/derived-default-combinations.scoop"
            )),
            &["nested", "enumNested", "closure"][..],
        ),
    ] {
        with_source(source, |output, mir| {
            let export = output.output().export.module();
            assert!(!mir.functions.is_empty());
            let mut snapshot = String::new();
            for name in names {
                let id = export
                    .functions
                    .iter()
                    .find(|(_, f)| f.name == *name)
                    .unwrap()
                    .0;
                let production = hir::DefaultSourceBodyProductionV1::from_dependency_hir(
                    output,
                    hir::ExportParameterOwner::Function(id),
                    2,
                    &mut BudgetMeter::new(DecodeLimits::default()),
                )
                .unwrap();
                let role = if *name == "closure" {
                    "Lambda"
                } else {
                    "DerivedEquality"
                };
                let references = production
                    .references()
                    .callables()
                    .iter()
                    .filter(|r| match r.target() {
                        hir::ExportDefaultCallableTargetV1::DerivedEquality { .. } => {
                            role == "DerivedEquality"
                        }
                        hir::ExportDefaultCallableTargetV1::Lambda { .. } => role == "Lambda",
                        _ => false,
                    })
                    .count();
                assert_eq!(references, 1, "{name}");
                snapshot.push_str(&format!("{name}: {role} 1\n"));
            }
            if names.len() == 2 {
                let (_, holder) = export
                    .classes
                    .iter()
                    .find(|(_, c)| c.name == "Holder")
                    .unwrap();
                let production = hir::DefaultSourceBodyProductionV1::from_dependency_hir(
                    output,
                    hir::ExportParameterOwner::ClassConstructor(holder.constructors[0]),
                    0,
                    &mut BudgetMeter::new(DecodeLimits::default()),
                )
                .unwrap();
                assert_eq!(
                    production
                        .references()
                        .callables()
                        .iter()
                        .filter(|r| matches!(
                            r.target(),
                            hir::ExportDefaultCallableTargetV1::DerivedEquality { .. }
                        ))
                        .count(),
                    1
                );
                snapshot.push_str("Holder: DerivedEquality 1\n");
            } else {
                assert_eq!(
                    export
                        .functions
                        .iter()
                        .filter(|(_, f)| f.name == "Atom.equals")
                        .count(),
                    1
                );
            }
            assert_eq!(
                snapshot,
                if names.len() == 2 {
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/derived-default-order.hir.snap"
                    ))
                } else {
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/derived-default-combinations.hir.snap"
                    ))
                }
            );
        });
    }
}
