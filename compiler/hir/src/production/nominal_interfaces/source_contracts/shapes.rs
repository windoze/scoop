use super::*;

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
    owner: SourceNominalId,
    binders: &[HirSignatureBinder],
    meter: &mut BudgetMeter,
) -> Result<(CanonicalSignatureTypesV1, NominalSourceShapeV1), Error> {
    let projection = source_shape::SourceShapeProjection::new(export);
    let (supertypes, shape) = match local {
        LocalNominalId::Class(id) => {
            let d = &export.classes[id];
            charge_types(
                export,
                d.base_class.iter().chain(&d.interfaces).copied(),
                binders.len(),
                meter,
            )?;
            (
                source_shape::class_supertypes(&projection, owner, d, binders),
                Ok(NominalSourceShapeV1::Class),
            )
        }
        LocalNominalId::Interface(id) => {
            let d = &export.interfaces[id];
            charge_types(
                export,
                d.parents
                    .iter()
                    .map(|id| export.interface_applications[*id].canonical_type),
                binders.len(),
                meter,
            )?;
            (
                source_shape::interface_supertypes(&projection, owner, d, binders),
                Ok(NominalSourceShapeV1::Interface),
            )
        }
        LocalNominalId::Struct(id) => {
            let d = &export.structs[id];
            charge_types(export, d.interfaces.iter().copied(), binders.len(), meter)?;
            charge_canonical(d.semantic_fields().len(), meter)?;
            for field in d.semantic_fields() {
                resources::ty(export, field.ty, binders.len(), 3, meter)?;
            }
            (
                source_shape::direct_supertypes(&projection, owner, &d.interfaces, binders),
                source_shape::struct_shape(&projection, id, d, binders, owner),
            )
        }
        LocalNominalId::Enum(id) => {
            let d = &export.enums[id];
            charge_types(export, d.interfaces.iter().copied(), binders.len(), meter)?;
            charge_canonical(d.variants.len(), meter)?;
            for variant in &d.variants {
                charge_canonical(variant.fields.len(), meter)?;
                for field in &variant.fields {
                    resources::ty(export, field.ty, binders.len(), 4, meter)?;
                }
            }
            (
                source_shape::direct_supertypes(&projection, owner, &d.interfaces, binders),
                source_shape::enum_shape(&projection, id, d, binders, owner),
            )
        }
        LocalNominalId::Object(id) => {
            let d = &export.objects[id];
            let backing = &export.classes[d.backing_class];
            charge_types(
                export,
                backing
                    .base_class
                    .iter()
                    .chain(&backing.interfaces)
                    .copied(),
                binders.len(),
                meter,
            )?;
            (
                source_shape::object_supertypes(&projection, owner, backing, binders),
                source_shape::object_shape(&projection, id, d, owner),
            )
        }
    };
    Ok((supertypes.map_err(invalid)?, shape.map_err(invalid)?))
}
fn charge_types(
    export: &ExportHir,
    types: impl Iterator<Item = TypeId> + Clone,
    binders: usize,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let count = types.clone().count();
    charge_canonical(count, meter)?;
    for ty in types {
        resources::ty(export, ty, binders, 3, meter)?;
    }
    Ok(())
}
