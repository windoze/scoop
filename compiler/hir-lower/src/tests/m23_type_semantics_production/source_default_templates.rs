use super::source_dispatch::with_hir_source;
use super::source_inventory::identity_closure;
use super::*;
use hir::{
    DecodedDefaultSourceTemplateV1 as Decoded, DefaultSourceBodyProductionV1 as Body,
    DefaultSourceTemplateV1 as Template,
};
use scoop_wire::{decode_canonical, decode_canonical_with_meter, encode};
mod budgets;
mod nested_occurrences;
mod public_receivers;
mod receivers;
mod rejection;
mod wire;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/templates.scoop"
));
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn function(export: &hir::ExportHir, name: &str) -> hir::ExportParameterOwner {
    hir::ExportParameterOwner::Function(
        export
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .0,
    )
}
fn template(output: &hir::DependencyHirOutput, name: &str, position: u32) -> Template {
    let mut shared = meter();
    Body::from_dependency_hir(
        output,
        function(output.output().export.module(), name),
        position,
        &mut shared,
    )
    .unwrap()
    .into_source_template(&mut shared)
    .unwrap()
}
fn bytes(value: &Template) -> Vec<u8> {
    encode(&value.index_locals(&mut meter()).unwrap()).unwrap()
}
fn round_trip(output: &hir::DependencyHirOutput, value: &Template) -> Template {
    let bytes = bytes(value);
    assert_eq!(&bytes[..2], &[0xac, 1]);
    let mut shared = meter();
    let input: Decoded = decode_canonical_with_meter(&bytes, &mut shared).unwrap();
    assert_eq!(encode(&input).unwrap(), bytes);
    let before = shared.usage();
    let restored = input
        .resolve(&mut identity_closure(output), &mut shared)
        .unwrap();
    assert_eq!(&restored, value);
    assert!(shared.usage().validation_work_units > before.validation_work_units);
    assert_eq!(
        encode(&restored.index_locals(&mut shared).unwrap()).unwrap(),
        bytes
    );
    restored
}

#[test]
fn source_template_bytes_restore_local_bindings_captures_and_original_provider() {
    with_hir_source(SOURCE, |output, _| {
        let mut summary = Vec::new();
        for (name, position) in [
            ("literal", 0),
            ("local", 1),
            ("SourceBase.choose", 1),
            ("SourceBase.callback", 1),
            ("SourceChild.choose", 1),
        ] {
            let value = template(output, name, position);
            let restored = round_trip(output, &value);
            let selectors = restored
                .locals()
                .records()
                .iter()
                .map(|local| format!("{:?}", local.selector()))
                .collect::<Vec<_>>();
            summary.push(format!(
                "{name}[{position}]: locals={}, parameters={}, binders={}, inherited={}",
                selectors.join(";"),
                restored.value_parameters().parameters().len(),
                restored.type_parameters().arguments().len(),
                restored.key().owner() != restored.definition_root().declaration()
            ));
            assert_eq!(bytes(&template(output, name, position)), bytes(&value));
        }
        assert_eq!(
            summary.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/templates.snap"
            ))
        );
        let parent = template(output, "SourceBase.choose", 1);
        let child = template(output, "SourceChild.choose", 1);
        assert_ne!(parent.key(), child.key());
        assert_eq!(parent.definition_root(), child.definition_root());
        assert_eq!(parent.definition_path(), child.definition_path());
        assert_eq!(parent.body(), child.body());
        assert_eq!(parent.references(), child.references());
    });
}

#[test]
fn source_template_conversion_moves_owned_leaves_without_reprojection() {
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "literal"),
            0,
            &mut meter(),
        )
        .unwrap();
        let hir::DefaultExpressionKindV1::StringLiteral { value, .. } = body.body().value().kind()
        else {
            panic!("literal required")
        };
        let pointer = value.as_ptr();
        let refs_pointer = body.references().types().as_ptr();
        let template = body.into_source_template(&mut meter()).unwrap();
        let hir::DefaultExpressionKindV1::StringLiteral { value, .. } =
            template.body().value().kind()
        else {
            panic!("literal required")
        };
        assert_eq!(value.as_ptr(), pointer);
        assert_eq!(template.references().types().as_ptr(), refs_pointer);
        round_trip(output, &template);
    });
}

#[test]
fn source_template_round_trips_all_reference_kinds_and_constructor_providers() {
    for source in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/references.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/bodies.scoop"
        )),
    ] {
        with_hir_source(source, |output, _| {
            for interface in &output.output().export.module().source_parameter_interfaces {
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
                    let mut shared = meter();
                    let value = Body::from_dependency_hir(
                        output,
                        interface.owner,
                        position as u32,
                        &mut shared,
                    )
                    .unwrap()
                    .into_source_template(&mut shared)
                    .unwrap();
                    round_trip(output, &value);
                }
            }
        });
    }
}

#[test]
fn source_template_preserves_generic_inherited_binder_substitution_in_bytes() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/inherited-generics.scoop"
    ));
    with_hir_source(source, |output, _| {
        let parent = round_trip(output, &template(output, "GenericParent.choose", 1));
        let child = round_trip(output, &template(output, "GenericChild.choose", 1));
        assert_eq!(parent.definition_root(), child.definition_root());
        assert_eq!(parent.result(), child.result());
        assert_eq!(parent.body(), child.body());
        assert_ne!(parent.type_parameters(), child.type_parameters());
        let scoop_identity::SignatureTypeKey::Tuple(elements) =
            &child.type_parameters().arguments()[0]
        else {
            panic!("tuple substitution required")
        };
        assert_eq!(
            elements.as_slice(),
            &[
                scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 },
                scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
            ]
        );
    });
}
