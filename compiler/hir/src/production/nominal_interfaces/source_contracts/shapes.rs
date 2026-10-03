use super::*;

pub(super) fn project(
    export: &ExportHir,
    local: LocalNominalId,
    owner: SourceNominalId,
    binders: &[HirSignatureBinder],
) -> Result<(CanonicalSignatureTypesV1, NominalSourceShapeV1), Error> {
    let projection = source_shape::SourceShapeProjection::new(export);
    let (supertypes, shape) = match local {
        LocalNominalId::Class(id) => {
            let d = &export.classes[id];

            (
                source_shape::class_supertypes(&projection, owner, d, binders),
                source_shape::class_shape(&projection, d, binders, owner),
            )
        }
        LocalNominalId::Interface(id) => {
            let d = &export.interfaces[id];

            (
                source_shape::interface_supertypes(&projection, owner, d, binders),
                Ok(NominalSourceShapeV1::Interface),
            )
        }
        LocalNominalId::Struct(id) => {
            let d = &export.structs[id];

            (
                source_shape::direct_supertypes(&projection, owner, &d.interfaces, binders),
                source_shape::struct_shape(&projection, id, d, binders, owner),
            )
        }
        LocalNominalId::Enum(id) => {
            let d = &export.enums[id];

            (
                source_shape::direct_supertypes(&projection, owner, &d.interfaces, binders),
                source_shape::enum_shape(&projection, id, d, binders, owner),
            )
        }
        LocalNominalId::Object(id) => {
            let d = &export.objects[id];
            let backing = &export.classes[d.backing_class];

            (
                source_shape::object_supertypes(&projection, owner, backing, binders),
                source_shape::object_shape(&projection, id, d, owner),
            )
        }
    };
    Ok((supertypes.map_err(invalid)?, shape.map_err(invalid)?))
}
