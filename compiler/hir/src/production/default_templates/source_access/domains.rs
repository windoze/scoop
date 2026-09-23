use super::*;
use scoop_identity::PersistentGenericTypeId;

impl DefaultSourceAccessDomainV1 {
    pub fn from_export_hir(
        export: &ExportHir,
        source: &AccessDomain,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(2, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        if source.is_empty() {
            return Ok(Self::empty());
        }
        let count = source.constraints().len();
        let mut persistent = Vec::new();
        let mut generic = Vec::new();
        meter.try_reserve_collection_slots(&mut persistent, count, &path)?;
        meter.try_reserve_collection_slots(&mut generic, count, &path)?;
        meter.charge_work(count as u64, &path)?;
        for constraint in source.constraints() {
            match constraint {
                AccessConstraint::Cone(id) => {
                    persistent.push(PersistentAccessConstraintV1::Cone(*id))
                }
                AccessConstraint::File(source) => {
                    let length = source.logical_path().as_str().len() as u64;
                    meter.check_semantic_leaf(length, &path)?;
                    meter.charge_owned_bytes(length, &path)?;
                    meter.charge_work(length, &path)?;
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
        charge_canonicalization(&persistent, generic.len(), meter)?;
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

fn charge_canonicalization(
    persistent: &[PersistentAccessConstraintV1],
    generic_count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    // The shared persistent-domain builder encodes sorting keys and checks order.
    for _ in 0..3 {
        meter.charge_collection_slots(persistent.len() as u64, &path)?;
    }
    for constraint in persistent {
        let length = scoop_wire::encoded_length(constraint).map_err(Error::Encoding)?;
        meter.charge_owned_bytes(length.saturating_mul(2), &path)?;
        meter.charge_work(
            length.saturating_mul(u64::from(persistent.len().max(1).ilog2()) + 4),
            &path,
        )?;
    }
    meter.check_table_entries(generic_count as u64, &path)?;
    meter.charge_work(
        (generic_count as u64).saturating_mul(u64::from(generic_count.max(1).ilog2()) + 1),
        &path,
    )?;
    Ok(())
}
