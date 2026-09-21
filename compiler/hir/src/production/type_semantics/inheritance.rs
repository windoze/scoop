use scoop_identity::PersistentExactTypeId;

use super::CrossConeTypeSemanticsProductionError as Error;
use super::nominals::{ConcreteNominal, NominalLocalId};
use crate::*;

mod constructors;
mod edges;
use edges::exact;
pub(super) use edges::{project_core_source_edges, project_edges};
mod schemas;
pub(super) use schemas::{interface_sources, slot_selections};
mod source_callables;
mod source_constructors;
pub(in crate::production::type_semantics) mod source_inventory;
mod source_parameters;
pub(in crate::production::type_semantics) mod source_properties;
mod source_protected_callables;
pub(in crate::production) mod source_resources;
pub(super) use source_callables::project as source_callables;
pub(super) use source_constructors::project as source_constructors;
pub(super) use source_inventory::project as source_inventory;
pub(super) use source_parameters::project as source_parameters;
pub(super) use source_properties::project as source_properties;
pub(super) use source_protected_callables::project as source_protected_callables;

type InheritanceProjection = (
    CanonicalNominalInheritanceInterfacesV1,
    CanonicalProtectedCallableSourceInterfacesV1,
    Vec<ExportDefinitionSourceV1>,
);

pub(super) fn produce(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    public_nominals: &CanonicalNominalInterfacesV1,
    public_callables: &CanonicalCallableInterfacesV1,
    public_sources: &CanonicalCallableSourceInterfacesV1,
    meter: &mut scoop_wire::BudgetMeter,
) -> Result<InheritanceProjection, Error> {
    let mut records = Vec::with_capacity(nominals.len());
    let mut origins = Vec::new();
    let mut sources = Vec::new();
    for nominal in nominals {
        let source_id = SourceNominalId::Concrete(nominal.owner);
        let public = public_nominals
            .get(source_id)
            .ok_or(Error::MissingLocalSupport(nominal.exact))?;
        let edge = project_edges(export, nominal)?;
        let domains = project_domains(export, nominal)?;
        let constructors = constructors::project(
            export,
            nominal,
            public,
            public_callables,
            public_sources,
            &mut sources,
            &mut origins,
        )?;
        let schemas = schemas::project(export, nominal, meter)?;
        let record = NominalInheritanceInterfaceV1::try_new(
            edge.clone(),
            domains,
            constructors,
            CanonicalInheritanceSlotContractsV1::try_new(Vec::new()).map_err(|error| {
                Error::InvalidInheritance {
                    exact: nominal.exact,
                    reason: error.to_string(),
                }
            })?,
            CanonicalProtectedDeclarationRefsV1::try_new(Vec::new()).map_err(|error| {
                Error::InvalidInheritance {
                    exact: nominal.exact,
                    reason: error.to_string(),
                }
            })?,
            schemas,
        )
        .map_err(|error| Error::InvalidInheritance {
            exact: nominal.exact,
            reason: error.to_string(),
        })?;
        records.push(record);
    }
    let table = CanonicalNominalInheritanceInterfacesV1::try_new(records).map_err(|error| {
        Error::InvalidTable {
            table: "inheritance",
            reason: error.to_string(),
        }
    })?;
    let sources =
        CanonicalProtectedCallableSourceInterfacesV1::try_new(sources).map_err(|error| {
            Error::InvalidTable {
                table: "protected-source-interface",
                reason: error.to_string(),
            }
        })?;
    Ok((table, sources, origins))
}

pub(super) fn reject_unsupported_source_features(
    export: &ExportHir,
    local: NominalLocalId,
    owner: SourceNominalId,
) -> Result<(), Error> {
    let has_dispatch = match local {
        NominalLocalId::Struct(id) => {
            methods_require_dispatch(export, &export.structs[id].methods)
                || properties_require_dispatch(export, &export.structs[id].properties)
        }
        NominalLocalId::Enum(id) => {
            methods_require_dispatch(export, &export.enums[id].methods)
                || properties_require_dispatch(export, &export.enums[id].properties)
        }
        NominalLocalId::Class(id) => {
            methods_require_dispatch(export, &export.classes[id].methods)
                || properties_require_dispatch(export, &export.classes[id].properties)
        }
        NominalLocalId::Interface(id) => {
            !export.interfaces[id].methods.is_empty()
                || !export.interfaces[id].private_methods.is_empty()
                || !export.interfaces[id].properties.is_empty()
        }
        NominalLocalId::Object(id) => {
            let class = &export.classes[export.objects[id].backing_class];
            methods_require_dispatch(export, &class.methods)
                || properties_require_dispatch(export, &class.properties)
        }
    };
    if has_dispatch {
        return Err(Error::UnsupportedDispatch(owner));
    }
    if constructors::has_protected(export, local) {
        Err(Error::UnsupportedProtectedConstructor(owner))
    } else {
        Ok(())
    }
}

fn methods_require_dispatch(export: &ExportHir, methods: &[FunctionId]) -> bool {
    methods.iter().any(|id| {
        let function = &export.functions[*id];
        function.access.declared == DeclaredVisibility::Protected
            || function
                .method
                .as_ref()
                .is_some_and(|method| !matches!(method.dispatch, MethodDispatch::Direct))
    })
}

fn properties_require_dispatch(export: &ExportHir, properties: &[PropertyId]) -> bool {
    properties.iter().any(|id| {
        let property = &export.properties[*id];
        property.access.declared == DeclaredVisibility::Protected
            || property.modifier != MethodModifier::Final
            || property.is_override
            || matches!(
                export.property_getters[property.capability.getter()].implementation,
                PropertyAccessorImplementation::AbstractSlot(_)
            )
            || property.capability.setter().is_some_and(|setter| {
                let setter = &export.property_setters[setter];
                setter.access.declared == DeclaredVisibility::Protected
                    || matches!(
                        setter.implementation,
                        PropertyAccessorImplementation::AbstractSlot(_)
                    )
            })
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
