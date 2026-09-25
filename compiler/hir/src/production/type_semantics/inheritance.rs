use scoop_identity::PersistentExactTypeId;

use super::CrossConeTypeSemanticsProductionError as Error;
use super::nominals::{ConcreteNominal, NominalLocalId};
use crate::*;

mod constructors;
mod edges;
use edges::exact;
pub(super) use edges::{project_edges, project_source_edges};
mod schemas;
mod slots;
pub(super) use schemas::{interface_sources, slot_selections};
pub(super) use slots::SlotContracts;
mod source_callables;
mod source_constructors;
pub(in crate::production) mod source_errors;
pub(in crate::production::type_semantics) mod source_inventory;
mod source_parameters;
pub(in crate::production::type_semantics) mod source_properties;
mod source_protected_callables;
pub(super) use source_callables::project as source_callables;
pub(super) use source_constructors::project as source_constructors;
pub(super) use source_inventory::project as source_inventory;
pub(super) use source_parameters::project as source_parameters;
pub(super) use source_properties::project as source_properties;
pub(super) use source_protected_callables::project as source_protected_callables;

pub(super) fn produce(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    source_constructors: &CanonicalInheritanceSourceConstructorsV1,
    slots: &SlotContracts<'_>,
) -> Result<CanonicalNominalInheritanceInterfacesV1, Error> {
    let mut records = Vec::with_capacity(nominals.len());
    for nominal in nominals {
        let source = inventory
            .get(nominal.exact)
            .ok_or(Error::MissingLocalSupport(nominal.exact))?;
        let record = NominalInheritanceInterfaceV1::try_new(
            project_edges(export, nominal)?,
            project_domains(export, nominal)?,
            constructors::project(nominal.exact, source, source_constructors)?,
            slots.project(nominal.exact, source.slot_schemas())?,
            source.protected_members().clone(),
            source.slot_schemas().clone(),
        )
        .map_err(|error| Error::InvalidInheritance {
            exact: nominal.exact,
            reason: error.to_string(),
        })?;
        records.push(record);
    }
    CanonicalNominalInheritanceInterfacesV1::try_new(records).map_err(|error| Error::InvalidTable {
        table: "inheritance",
        reason: error.to_string(),
    })
}

fn project_domains(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<NominalAccessDomainsV1, Error> {
    let (access, inheritance_kind) = match nominal.local {
        NominalLocalId::Struct(id) => (&export.structs[id].access, InheritanceDomainKind::Final),
        NominalLocalId::Enum(id) => (&export.enums[id].access, InheritanceDomainKind::Final),
        NominalLocalId::Class(id) => (
            &export.classes[id].access,
            match export.classes[id].modifier {
                ClassModifier::Final => InheritanceDomainKind::Final,
                ClassModifier::Open | ClassModifier::Abstract => InheritanceDomainKind::Class,
            },
        ),
        NominalLocalId::Interface(id) => (
            &export.interfaces[id].access,
            InheritanceDomainKind::Interface,
        ),
        NominalLocalId::Object(id) => (&export.objects[id].access, InheritanceDomainKind::Final),
    };
    let lookup = domain(export, &access.lookup.0)?;
    let inheritance = match inheritance_kind {
        InheritanceDomainKind::Final => PersistentAccessDomainV1::empty(),
        InheritanceDomainKind::Interface => lookup.clone(),
        InheritanceDomainKind::Class => lookup
            .intersect(
                &PersistentAccessDomainV1::try_from_constraints(vec![
                    PersistentAccessConstraintV1::SubclassesOf(nominal.exact),
                ])
                .map_err(|error| Error::InvalidTable {
                    table: "inheritance-domain",
                    reason: error.to_string(),
                })?,
            )
            .map_err(|error| Error::InvalidTable {
                table: "inheritance-domain",
                reason: error.to_string(),
            })?,
    };
    Ok(NominalAccessDomainsV1::new(
        PersistentLookupDomainV1::new(lookup),
        PersistentInheritanceDomainV1::new(inheritance),
        PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::empty()),
    ))
}

#[derive(Clone, Copy)]
enum InheritanceDomainKind {
    Final,
    Class,
    Interface,
}

fn domain(export: &ExportHir, source: &AccessDomain) -> Result<PersistentAccessDomainV1, Error> {
    if source.is_empty() {
        return Ok(PersistentAccessDomainV1::empty());
    }
    let constraints = source
        .constraints()
        .iter()
        .map(|constraint| match constraint {
            AccessConstraint::Cone(cone) => Ok(PersistentAccessConstraintV1::Cone(*cone)),
            AccessConstraint::File(file) => Ok(PersistentAccessConstraintV1::File(file.clone())),
            AccessConstraint::LexicalOwner(owner) => {
                persistent_owner(export, *owner).map(PersistentAccessConstraintV1::LexicalOwner)
            }
            AccessConstraint::SubclassesOf(class) => {
                let ty = export.class_applications[export.classes[*class].self_application]
                    .canonical_type;
                exact(export, ty).map(PersistentAccessConstraintV1::SubclassesOf)
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;
    PersistentAccessDomainV1::try_from_constraints(constraints).map_err(|error| {
        Error::InvalidTable {
            table: "access-domain",
            reason: error.to_string(),
        }
    })
}

fn persistent_owner(export: &ExportHir, owner: VisibilityOwner) -> Result<SourceNominalId, Error> {
    let identity = match owner {
        VisibilityOwner::Class(id) => &export.nominal_identities[id],
        VisibilityOwner::Interface(id) => &export.nominal_identities[id],
        VisibilityOwner::Struct(id) => &export.nominal_identities[id],
        VisibilityOwner::Enum(id) => &export.nominal_identities[id],
        VisibilityOwner::Object(id) => &export.nominal_identities[id],
    };
    let source = identity.source().ok_or(Error::MissingExactIdentity)?;
    Ok(match source {
        HirSourceNominalIdentity::Concrete(record) => SourceNominalId::Concrete(record.id()),
        HirSourceNominalIdentity::Generic(record) => SourceNominalId::GenericTemplate(record.id()),
    })
}
