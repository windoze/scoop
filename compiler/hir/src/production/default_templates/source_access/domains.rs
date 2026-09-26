use super::*;

pub(super) fn nominal(
    export: &ExportHir,
    owner: VisibilityOwner,
) -> Result<SourceNominalId, Error> {
    let identity = match owner {
        VisibilityOwner::Class(id) => export.nominal_identities.get_class(id),
        VisibilityOwner::Interface(id) => export.nominal_identities.get_interface(id),
        VisibilityOwner::Struct(id) => export.nominal_identities.get_struct(id),
        VisibilityOwner::Enum(id) => export.nominal_identities.get_enum(id),
        VisibilityOwner::Object(id) => export.nominal_identities.get_object(id),
    }
    .and_then(HirNominalIdentity::source)
    .ok_or(Error::MissingNominal(owner))?;
    Ok(match identity {
        HirSourceNominalIdentity::Concrete(record) => SourceNominalId::Concrete(record.id()),
        HirSourceNominalIdentity::Generic(record) => SourceNominalId::GenericTemplate(record.id()),
    })
}
