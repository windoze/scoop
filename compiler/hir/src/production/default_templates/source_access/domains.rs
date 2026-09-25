use super::*;
use scoop_identity::PersistentGenericTypeId;

impl DefaultSourceAccessDomainV1 {
    pub fn from_export_hir(export: &ExportHir, source: &AccessDomain) -> Result<Self, Error> {
        let path = WirePath::root();

        if source.is_empty() {
            return Ok(Self::empty());
        }
        let count = source.constraints().len();
        let mut persistent = Vec::new();
        let mut generic = Vec::new();
        scoop_wire::allocation::try_reserve(&mut persistent, count, &path)?;
        scoop_wire::allocation::try_reserve(&mut generic, count, &path)?;

        for constraint in source.constraints() {
            match constraint {
                AccessConstraint::Cone(id) => {
                    persistent.push(PersistentAccessConstraintV1::Cone(*id))
                }
                AccessConstraint::File(source) => {
                    persistent.push(PersistentAccessConstraintV1::File(source.clone()));
                }
                AccessConstraint::LexicalOwner(owner) => persistent.push(
                    PersistentAccessConstraintV1::LexicalOwner(nominal(export, *owner)?),
                ),
                AccessConstraint::SubclassesOf(class) => {
                    match nominal(export, VisibilityOwner::Class(*class))? {
                        SourceNominalId::GenericTemplate(id) => generic.push(id),
                        SourceNominalId::Concrete(_) => {
                            let class_data = super::super::arena_get(&export.classes, *class)
                                .ok_or(Error::MissingNominal(VisibilityOwner::Class(*class)))?;
                            let application = super::super::arena_get(
                                &export.class_applications,
                                class_data.self_application,
                            )
                            .ok_or(Error::MissingExact(*class))?;
                            let exact = export
                                .type_identities
                                .get(application.canonical_type)
                                .and_then(HirTypeIdentity::exact)
                                .ok_or(Error::MissingExact(*class))?;
                            persistent.push(PersistentAccessConstraintV1::SubclassesOf(exact.id()));
                        }
                    }
                }
            }
        }

        let persistent =
            PersistentAccessDomainV1::try_from_constraints(persistent).map_err(Error::Domain)?;
        let generic = CanonicalPersistentIdsV1::<PersistentGenericTypeId>::try_new(generic)
            .map_err(Error::GenericSubclasses)?;
        Self::try_new(persistent, generic).map_err(Error::Build)
    }
}

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
