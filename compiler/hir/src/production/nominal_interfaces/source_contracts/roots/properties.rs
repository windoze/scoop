use super::*;

pub(super) fn visit_types(
    export: &ExportHir,
    local: LocalNominalId,
    mut visit: impl FnMut(TypeId) -> Result<(), Error>,
) -> Result<(), Error> {
    let properties = match local {
        LocalNominalId::Class(id) => &export.classes[id].properties,
        LocalNominalId::Struct(id) => &export.structs[id].properties,
        LocalNominalId::Enum(id) => &export.enums[id].properties,
        LocalNominalId::Interface(id) => &export.interfaces[id].properties,
        LocalNominalId::Object(id) => &export.classes[export.objects[id].backing_class].properties,
    };
    for property in properties {
        visit(export.properties[*property].ty)?;
    }
    Ok(())
}
