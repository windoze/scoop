use super::*;
use hir::{DefaultSourceTemplateV1 as Template, ExportDefaultReferenceKindV1 as Kind};
pub(super) const INDEPENDENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/reference-closure.scoop"
));
pub(super) fn with_template(run: impl FnOnce(&Template)) {
    with_hir_source(INDEPENDENT, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "ReferenceClosureHost.all"),
            0,
            &mut meter(),
        )
        .unwrap();
        let template = body.into_source_template(&mut meter()).unwrap();
        let bytes = encode(&template.index_locals(&mut meter()).unwrap()).unwrap();
        let decoded: hir::DecodedDefaultSourceTemplateV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let restored = decoded
            .resolve(&mut identity_closure(output), &mut meter())
            .unwrap();
        run(&restored);
    });
}
pub(super) fn rebuild(t: &Template, references: References) -> Template {
    Template::try_new(
        t.key(),
        t.definition_root(),
        t.definition_path().clone(),
        t.locals().clone(),
        t.body().clone(),
        t.result().clone(),
        t.allows_suspend(),
        t.type_parameters().clone(),
        t.receiver().clone(),
        t.value_parameters().clone(),
        references,
        t.definition_origin().clone(),
        &mut meter(),
    )
    .unwrap()
}
pub(super) fn changed(t: &Template, kind: Kind, change: Change) -> Template {
    let r = t.references();
    let mut callables = r.callables().to_vec();
    let mut constructors = r.constructors().to_vec();
    let mut types = r.types().to_vec();
    let mut globals = r.globals().to_vec();
    let mut singletons = r.singleton_values().to_vec();
    let mut fields = r.fields().to_vec();
    match kind {
        Kind::Callable => mutate(&mut callables, change),
        Kind::Constructor => mutate(&mut constructors, change),
        Kind::Type => mutate(&mut types, change),
        Kind::Global => mutate(&mut globals, change),
        Kind::Singleton => mutate(&mut singletons, change),
        Kind::Field => mutate(&mut fields, change),
    }
    rebuild(
        t,
        References::try_new(callables, constructors, types, globals, singletons, fields).unwrap(),
    )
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Change {
    Missing,
    Extra,
    Reordered,
    Origin,
    Target,
}
fn mutate<T: Clone + PartialEq>(
    records: &mut Vec<hir::DefaultSourceReferenceV1<T>>,
    change: Change,
) {
    assert!(records.len() >= 2);
    match change {
        Change::Missing => {
            records.pop();
        }
        Change::Extra => records.push(records[0].clone()),
        Change::Reordered => {
            let index = records
                .iter()
                .position(|r| r.definition_origin() != records[0].definition_origin())
                .unwrap();
            records.swap(0, index);
        }
        Change::Origin => {
            let origin = records
                .iter()
                .find(|r| r.definition_origin() != records[0].definition_origin())
                .unwrap()
                .definition_origin()
                .clone();
            records[0] = hir::DefaultSourceReferenceV1::new(
                records[0].target().clone(),
                origin,
                records[0].witness().clone(),
            );
        }
        Change::Target => {
            let target = records
                .iter()
                .find(|r| r.target() != records[0].target())
                .unwrap()
                .target()
                .clone();
            records[0] = hir::DefaultSourceReferenceV1::new(
                target,
                records[0].definition_origin().clone(),
                records[0].witness().clone(),
            );
        }
    }
}
