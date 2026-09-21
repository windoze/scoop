use super::*;
mod rejection;
mod resources;
mod snapshots;
mod support;
use support::*;

#[test]
fn source_reference_closure_matches_every_collector_occurrence_in_order() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-type-source-defaults");
    for name in [
        "reference-closure",
        "reference-closure-combinations",
        "bodies",
        "data-flow",
        "nested-index",
        "templates",
        "table",
        "inherited-generics",
        "nested-local-captures",
        "generic-closure-expansion",
        "generic-reference-owner",
        "origin-binding",
        "nested-parents",
        "nested-local-calls",
        "nested-occurrences",
        "local-owner-arguments",
        "repeated-default-closures",
        "nested-capture-combinations",
        "references",
        "access",
        "declaration-binding",
        "nested-identities",
        "type-envelope",
        "local-own-binders",
        "local-own-binder-combinations",
        "local-dependency-combinations",
    ] {
        let source = std::fs::read_to_string(root.join(format!("{name}.scoop"))).unwrap();
        with_hir_source(&source, |output, _| {
            let export = output.output().export.module();
            for interface in &export.source_parameter_interfaces {
                for (position, parameter) in interface.parameters.iter().enumerate() {
                    if !matches!(
                        parameter.calling,
                        hir::ExportParameterCalling::Default { .. }
                            | hir::ExportParameterCalling::Vararg {
                                omission: hir::ExportVarargOmission::Default(_),
                                ..
                            }
                    ) {
                        continue;
                    }
                    let template = Body::from_dependency_hir(
                        output,
                        interface.owner,
                        position as u32,
                        &mut meter(),
                    )
                    .unwrap()
                    .into_source_template(&mut meter())
                    .unwrap();
                    let bound = template
                        .bind_reference_occurrences(&mut meter(), &scoop_wire::WirePath::root())
                        .unwrap_or_else(|e| {
                            panic!(
                                "{name}, owner {:?}, position {position}: {e:?}",
                                interface.owner
                            )
                        });
                    assert_eq!(bound.template(), template.key());
                    assert_eq!(
                        bound.occurrences().len(),
                        counts(template.references()).iter().sum()
                    );
                    for occurrence in bound.occurrences() {
                        assert_eq!(
                            occurrence.source().definition_origin(),
                            occurrence.body().definition_origin
                        );
                    }
                }
            }
        });
    }
}

#[test]
fn source_reference_closure_preserves_array_assembly_order() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/reference-closure-arrays.scoop"
    ));
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let export = output.export.module();
    let template = Body::from_export_hir(
        export,
        function(export, "ClosureArrayHost.arrays"),
        1,
        &mut meter(),
    )
    .unwrap()
    .into_source_template(&mut meter())
    .unwrap();
    let bound = template
        .bind_reference_occurrences(&mut meter(), &WirePath::root())
        .unwrap();
    assert_eq!(
        bound.occurrences().len(),
        counts(template.references()).iter().sum()
    );
}
