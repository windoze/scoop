use super::*;
use std::collections::BTreeSet;
use std::fmt::Write;

pub(super) fn materializations(
    input: &scoop_mir::ConeMirInput,
    records: &CanonicalParamFreeMirTypeExportsV1,
) {
    let module = input.module();
    let required = input
        .materialization()
        .shape_support()
        .iter()
        .map(|root| root.shape().exact())
        .collect::<BTreeSet<_>>();
    for root in input.materialization().generated_nominal_shapes() {
        let identity = module
            .meta
            .generated_exact_types
            .get(root.location())
            .unwrap();
        let subject = match identity.nominal_record().key() {
            GeneratedNominalKey::BoxedValue { payload } => *payload,
            GeneratedNominalKey::CoroutineStep { result } => *result,
            GeneratedNominalKey::CoroutineSlot { value } => *value,
            _ => {
                assert!(records.get(root.exact()).is_none());
                continue;
            }
        };
        let found = records.get(root.exact());
        assert_eq!(found.is_some(), required.contains(&subject));
        let Some(found) = found else {
            continue;
        };
        assert_eq!(
            found.origin(),
            &MirTypeOriginV1::GeneratedNominal {
                nominal: identity.nominal_record().id(),
                role: identity.nominal_record().key().clone()
            }
        );
        match (root.location(), found.representation()) {
            (
                scoop_mir::GeneratedExactTypeLocation::Enum(id),
                MirTypeRepresentationV1::CoroutineStep { variants }
                | MirTypeRepresentationV1::CoroutineSlot { variants },
            ) => {
                assert_eq!(variants.len(), module.enums[id].variants.len());
                for (actual, produced) in module.enums[id].variants.iter().zip(variants) {
                    assert_eq!(actual.identity, produced.variant);
                    assert_eq!(
                        actual.gc_free,
                        produced.gc == scoop_mir::MirGcKindV1::GcFree
                    );
                    assert_eq!(actual.fields.len(), produced.fields.len());
                    for (actual, produced) in actual.fields.iter().zip(&produced.fields) {
                        assert_eq!(actual.identity, produced.field);
                        assert_eq!(
                            module
                                .meta
                                .source_exact_types
                                .get(&actual.ty)
                                .unwrap()
                                .identity_record()
                                .id(),
                            produced.value
                        );
                    }
                }
            }
            (
                scoop_mir::GeneratedExactTypeLocation::Class(id),
                MirTypeRepresentationV1::BoxedValue { payload },
            ) => {
                let boxed = module
                    .meta
                    .boxed_types
                    .iter()
                    .find(|boxed| boxed.class() == id)
                    .unwrap();
                assert_eq!(payload.field, boxed.identity().payload_field_record().id());
                assert_eq!(
                    payload.value,
                    module
                        .meta
                        .source_exact_types
                        .get(&module.classes[id].declared_fields()[0].ty)
                        .unwrap()
                        .identity_record()
                        .id()
                );
            }
            _ => panic!("finite roots preserve their exact representation role"),
        }
    }
}

pub(super) fn projection(
    input: &scoop_mir::ConeMirInput,
    records: &CanonicalParamFreeMirTypeExportsV1,
    count: usize,
) -> String {
    let mut rows = Vec::new();
    for root in input.materialization().shape_support() {
        let name = scoop_mir::type_name(input.module(), root.shape().ty());
        if !name.starts_with("Finite") {
            continue;
        }
        for record in records.records() {
            let MirTypeOriginV1::GeneratedNominal { role, .. } = record.origin() else {
                panic!("generated export")
            };
            let (role, subject) = match role {
                GeneratedNominalKey::BoxedValue { payload } => ("box", payload),
                GeneratedNominalKey::CoroutineStep { result } => ("step", result),
                GeneratedNominalKey::CoroutineSlot { value } => ("slot", value),
                _ => panic!("finite role"),
            };
            if *subject != root.shape().exact() {
                continue;
            }
            let mut row = format!("{name} {role}: {} {:?}\n", record.exact(), record.facts());
            for field in record.representation().fields() {
                writeln!(row, "  field {}: {}", field.field, field.value).unwrap();
            }
            for variant in record.representation().variants() {
                writeln!(row, "  variant {} {:?}", variant.variant, variant.gc).unwrap();
                for field in &variant.fields {
                    writeln!(row, "    field {}: {}", field.field, field.value).unwrap();
                }
            }
            for interface in &record.base_and_interfaces().interfaces {
                writeln!(row, "  interface {interface}").unwrap();
            }
            rows.push(row);
        }
    }
    assert_eq!(rows.len(), count);
    rows.sort();
    rows.concat()
}

pub(super) fn hidden_box(
    input: &scoop_mir::ConeMirInput,
    records: &CanonicalParamFreeMirTypeExportsV1,
) {
    let module = input.module();
    let boxed = module.meta.boxed_types.iter().find(|boxed| {
        matches!(boxed.payload(), scoop_mir::Type::Struct(id) if module.structs[*id].name == "FiniteHidden")
    }).expect("the public body really materializes its private payload box");
    let generated = module
        .meta
        .generated_exact_types
        .get(scoop_mir::GeneratedExactTypeLocation::Class(boxed.class()))
        .unwrap();
    assert!(records.get(generated.exact_record().id()).is_none());
}
