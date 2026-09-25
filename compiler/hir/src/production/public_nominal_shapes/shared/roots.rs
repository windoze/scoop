use super::*;
use scoop_identity::BindableEntity;

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

pub(super) fn current(
    export: &ExportHir,
) -> Result<Vec<SourceNominalId>, PublicNominalShapeProjectionError> {
    let path = WirePath::root();
    let mut keys = BTreeMap::new();
    for record in export.export_binding_identities.iter() {
        keys.insert(record.id(), record.key());
    }
    let mut roots = BTreeSet::new();
    for record in export.public_export_bindings.records() {
        let key = keys.get(&record.binding()).ok_or(
            PublicNominalShapeProjectionError::MissingBinding(record.binding()),
        )?;
        if key.exporter() != export.cone {
            continue;
        }
        let ExportBindingSourceV1::DeclaredCurrent { declaration } = record.source() else {
            continue;
        };
        let source = match declaration {
            BindableEntity::Type(id) => SourceNominalId::Concrete(*id),
            BindableEntity::GenericType(id) => SourceNominalId::GenericTemplate(*id),
            _ => continue,
        };

        if !roots.contains(&source) {
            roots.insert(source);
        }
    }
    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, roots.len(), &path).map_err(resource)?;
    values.extend(roots);
    Ok(values)
}
