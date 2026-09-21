use super::source_dispatch::with_hir_source;
use super::source_inventory::identity_closure;
use super::*;
use hir::{
    DecodedDefaultSourceReferencesV1 as Decoded, DefaultSourceBodyProductionV1 as Body,
    DefaultSourceReferencesV1 as References,
};
use scoop_wire::{decode_canonical, encode};
mod budgets;
mod closure;
mod wire;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/references.scoop"
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
fn counts(r: &References) -> [usize; 6] {
    [
        r.callables().len(),
        r.constructors().len(),
        r.types().len(),
        r.globals().len(),
        r.singleton_values().len(),
        r.fields().len(),
    ]
}
fn raw_counts(r: &hir::ExportDefaultReferences) -> [usize; 6] {
    [
        r.callables.len(),
        r.constructors.len(),
        r.types.len(),
        r.globals.len(),
        r.singleton_values.len(),
        r.fields.len(),
    ]
}

#[test]
fn source_reference_occurrences_cover_six_kinds_and_round_trip_real_foundation() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let mut summary = Vec::new();
        let mut totals = [0; 6];
        for name in [
            "callable",
            "constructor",
            "variant",
            "classConstructor",
            "global",
            "singleton",
            "field",
            "combined",
        ] {
            let body =
                Body::from_dependency_hir(output, function(export, name), 0, &mut meter()).unwrap();
            let refs = body.references();
            let actual = counts(refs);
            assert_eq!(actual, raw_counts(body.source_references()));
            for (total, count) in totals.iter_mut().zip(actual) {
                *total += count;
            }
            let bytes = encode(refs).unwrap();
            let decoded: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            assert_eq!(encode(&decoded).unwrap(), bytes);
            let restored = decoded
                .resolve(&mut identity_closure(output), &mut meter())
                .unwrap();
            assert_eq!(&restored, refs);
            assert_eq!(encode(&restored).unwrap(), bytes);
            let again =
                Body::from_dependency_hir(output, function(export, name), 0, &mut meter()).unwrap();
            assert_eq!(again.references(), refs);
            summary.push(format!("{name}: {actual:?}"));
        }
        assert!(totals.into_iter().all(|n| n > 0));
        assert_eq!(
            summary.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/references.snap"
            ))
        );
    });
}

#[test]
fn source_references_preserve_callable_order_multiplicity_and_each_origin() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let body = Body::from_dependency_hir(output, function(export, "combined"), 0, &mut meter())
            .unwrap();
        let refs = body.references().callables();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].target(), refs[2].target());
        assert_ne!(refs[0].target(), refs[1].target());
        assert_ne!(refs[0].definition_origin(), refs[2].definition_origin());
        let raw = &body.source_references().callables;
        for (record, source) in refs.iter().zip(raw) {
            assert_eq!(
                record.witness().owner(),
                body.definition_root().declaration()
            );
            let origin = scoop_identity::DefinitionOrigin::new(
                export.source_files[source.origin.file as usize]
                    .identity
                    .clone(),
                scoop_identity::SourceSpan::new(
                    u64::from(source.origin.span.start),
                    u64::from(source.origin.span.end),
                )
                .unwrap(),
                export
                    .source_context_identities
                    .get(source.origin.context)
                    .unwrap()
                    .key(),
            )
            .unwrap();
            assert_eq!(record.definition_origin().origin(), &origin);
            assert_eq!(
                record.witness(),
                &hir::DefaultSourceAccessWitnessV1::from_export_hir(
                    export,
                    &source.witness,
                    &mut meter()
                )
                .unwrap()
            );
        }
    });
}

#[test]
fn inherited_and_generic_source_references_keep_original_providers() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-defaults/access.scoop"
    ));
    with_hir_source(source, |output, _| {
        let export = output.output().export.module();
        let parent = Body::from_dependency_hir(
            output,
            function(export, "LocalBase.choose"),
            0,
            &mut meter(),
        )
        .unwrap();
        let child = Body::from_dependency_hir(
            output,
            function(export, "LocalChild.choose"),
            0,
            &mut meter(),
        )
        .unwrap();
        assert_ne!(child.owner(), parent.owner());
        assert_eq!(child.references(), parent.references());
        for name in ["LocalBase.hidden", "LocalChild.choose", "Generic.choose"] {
            let body =
                Body::from_dependency_hir(output, function(export, name), 0, &mut meter()).unwrap();
            let decoded: Decoded =
                decode_canonical(&encode(body.references()).unwrap(), DecodeLimits::default())
                    .unwrap();
            assert_eq!(
                decoded
                    .resolve(&mut identity_closure(output), &mut meter())
                    .unwrap(),
                *body.references()
            );
            for record in body.references().types() {
                assert_eq!(
                    record.witness().owner(),
                    body.definition_root().declaration()
                );
            }
        }
    });
}

#[test]
fn source_producer_rejects_reference_provider_from_another_body() {
    with_hir_source(SOURCE, |output, _| {
        let mut export = output.output().export.module().clone();
        let owner = function(&export, "callable");
        let wrong = function(&export, "global");
        let template = export
            .export_default_exprs
            .iter_mut()
            .find(|(_, expr)| {
                expr.references
                    .callables
                    .iter()
                    .any(|r| r.witness.owner == owner)
            })
            .unwrap()
            .1;
        template.references.callables[0].witness.owner = wrong;
        let error = Body::from_export_hir(&export, owner, 0, &mut meter()).unwrap_err();
        assert!(
            matches!(
                error,
                hir::DefaultSourceBodyProductionError::References(
                    hir::DefaultSourceReferencesProductionError::Provider {
                        kind: hir::ExportDefaultReferenceKindV1::Callable,
                        index: 0,
                        ..
                    }
                )
            ),
            "{error:?}"
        );
    });
}
