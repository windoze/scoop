use super::*;

pub(super) fn all_current(
    export: &ExportHir,
) -> Result<Vec<SourceNominalId>, PublicNominalShapeProjectionError> {
    let path = WirePath::root();
    let declarations = export
        .structs
        .iter()
        .map(|(id, _)| &export.nominal_identities[id])
        .chain(
            export
                .enums
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        )
        .chain(
            export
                .classes
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        )
        .chain(
            export
                .interfaces
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        )
        .chain(
            export
                .objects
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        );
    let mut roots = Vec::new();
    for identity in declarations {
        let Some(source) = identity.source() else {
            continue;
        };
        if source.declaration().origin() != export.cone {
            continue;
        }
        let source = match source {
            crate::HirSourceNominalIdentity::Concrete(record) => {
                SourceNominalId::Concrete(record.id())
            }
            crate::HirSourceNominalIdentity::Generic(record) => {
                SourceNominalId::GenericTemplate(record.id())
            }
        };

        scoop_wire::allocation::try_reserve(&mut roots, 1, &path).map_err(resource)?;
        roots.push(source);
    }
    Ok(roots)
}
