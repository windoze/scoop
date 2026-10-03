use super::*;

pub(in crate::cross_cone_types) fn exact(
    module: &mir::Module,
    ty: &mir::Type,
) -> Result<PersistentExactTypeId, SourceMirTypeProductionError> {
    module
        .meta
        .source_exact_types
        .get(ty)
        .map(|identity| identity.identity_record().id())
        .ok_or(SourceMirTypeProductionError::MissingFieldType)
}

pub(super) fn class(
    module: &mir::Module,
    class: &mir::ClassDef,
    owner: PersistentTypeId,
    expected: &[hir::ClassRepresentationFieldV1],
) -> Result<Vec<mir::MirRepresentationFieldV1>, SourceMirTypeProductionError> {
    let mismatch = || SourceMirTypeProductionError::RepresentationMismatch(owner);
    let mir::ClassRepresentation::Declared { fields, base_class } = &class.representation else {
        return Err(mismatch());
    };
    let inherited = base_class.map_or(0, |id| module.classes[id].declared_fields().len());
    let fields = fields.get(inherited..).ok_or_else(mismatch)?;
    if fields.len() != expected.len() {
        return Err(mismatch());
    }
    let mut projected = Vec::new();
    reserve(&mut projected, fields.len())?;
    for (field, expected) in fields.iter().zip(expected) {
        projected.push(mir::MirRepresentationFieldV1 {
            field: expected.field(),
            value: exact(module, &field.ty)?,
        });
    }
    Ok(projected)
}

pub(in crate::cross_cone_types) fn variants(
    module: &mir::Module,
    variants: &[mir::VariantDef],
) -> Result<Vec<mir::MirRepresentationVariantV1>, SourceMirTypeProductionError> {
    let mut projected = Vec::new();
    reserve(&mut projected, variants.len())?;
    for variant in variants {
        let mut fields = Vec::new();
        reserve(&mut fields, variant.fields.len())?;
        for field in &variant.fields {
            fields.push(mir::MirRepresentationVariantFieldV1 {
                field: field.identity,
                value: exact(module, &field.ty)?,
            });
        }
        projected.push(mir::MirRepresentationVariantV1 {
            variant: variant.identity,
            fields,
            gc: if variant.gc_free {
                mir::MirGcKindV1::GcFree
            } else {
                mir::MirGcKindV1::ContainsManagedReferences
            },
        });
    }
    Ok(projected)
}
