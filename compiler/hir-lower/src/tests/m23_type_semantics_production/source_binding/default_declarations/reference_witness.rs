use super::*;
use hir::{
    DefaultSourceAccessWitnessV1 as Witness, DefaultSourceReferenceV1 as Reference,
    ExportDefaultReferenceKindV1 as Kind,
};

pub(in crate::tests::m23_type_semantics_production::source_binding) fn change(
    t: &Template,
    kind: Kind,
    update: impl FnOnce(&Witness) -> Witness,
    output: &hir::OrdinaryHirOutput,
) -> Template {
    change_at(t, kind, 0, update, output)
}

pub(in crate::tests::m23_type_semantics_production::source_binding) fn change_at(
    t: &Template,
    kind: Kind,
    index: usize,
    update: impl FnOnce(&Witness) -> Witness,
    output: &hir::OrdinaryHirOutput,
) -> Template {
    let refs = t.references();
    let mut callables = refs.callables().to_vec();
    let mut constructors = refs.constructors().to_vec();
    let mut types = refs.types().to_vec();
    let mut globals = refs.globals().to_vec();
    let mut singletons = refs.singleton_values().to_vec();
    let mut fields = refs.fields().to_vec();
    match kind {
        Kind::Callable => overwrite(&mut callables[index], update),
        Kind::Constructor => overwrite(&mut constructors[index], update),
        Kind::Type => overwrite(&mut types[index], update),
        Kind::Global => overwrite(&mut globals[index], update),
        Kind::Singleton => overwrite(&mut singletons[index], update),
        Kind::Field => overwrite(&mut fields[index], update),
    }
    let refs = hir::DefaultSourceReferencesV1::try_new(
        callables,
        constructors,
        types,
        globals,
        singletons,
        fields,
    )
    .unwrap();
    let changed = Template::try_new(
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
        refs,
        t.definition_origin().clone(),
        &mut meter(),
    )
    .unwrap();
    let bytes = encode(&changed.index_locals(&mut meter()).unwrap()).unwrap();
    let decoded: hir::DecodedDefaultSourceTemplateV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    decoded
        .resolve(&mut identity_closure(output), &mut meter())
        .unwrap()
}
fn overwrite<T: Clone>(record: &mut Reference<T>, update: impl FnOnce(&Witness) -> Witness) {
    *record = Reference::new(
        record.target().clone(),
        record.definition_origin().clone(),
        update(record.witness()),
    );
}
