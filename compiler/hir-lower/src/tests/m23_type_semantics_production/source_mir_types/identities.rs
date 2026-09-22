use super::*;

pub(super) fn source_members(
    output: &hir::DependencyHirOutput,
    table: &CanonicalParamFreeMirTypeExportsV1,
) {
    let export = output.output().export.module();
    let exact = |ty| export.type_identities[ty].exact().unwrap().id();
    let mut fields = BTreeMap::new();
    for (id, source) in export.structs.iter() {
        for (index, field) in source.semantic_fields().iter().enumerate() {
            let reference =
                hir::StructFieldRef::checked(&export.structs, id, index as u32).unwrap();
            fields.insert(export.field_identities[reference].id(), exact(field.ty));
        }
    }
    for (id, field) in export.class_fields.iter() {
        fields.insert(export.field_identities[id].id(), exact(field.ty));
    }
    let mut variants = BTreeMap::new();
    for (id, source) in export.enums.iter() {
        for (index, variant) in source.variants.iter().enumerate() {
            let reference = hir::EnumVariantRef::checked(&export.enums, id, index as u32).unwrap();
            let values = variant
                .fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    let reference =
                        hir::EnumVariantFieldRef::checked(&export.enums, reference, index as u32)
                            .unwrap();
                    (
                        export.enum_member_identities[reference].id(),
                        exact(field.ty),
                    )
                })
                .collect::<Vec<_>>();
            variants.insert(export.enum_member_identities[reference].id(), values);
        }
    }
    for record in table.records() {
        for field in record.representation().fields() {
            assert_eq!(fields[&field.field], field.value);
        }
        for variant in record.representation().variants() {
            assert_eq!(
                variants[&variant.variant],
                variant
                    .fields
                    .iter()
                    .map(|field| (field.field, field.value))
                    .collect::<Vec<_>>()
            );
        }
    }
}
