use scoop_identity::PersistentExactTypeId;

use super::CrossConeTypeSemanticsProductionError as Error;
use super::nominals::{ConcreteNominal, NominalLocalId};
use crate::*;

mod constructors;
mod schemas;
mod source_inventory;
pub(super) use source_inventory::project as source_inventory;

type InheritanceProjection = (
    CanonicalNominalInheritanceInterfacesV1,
    Vec<NominalInheritanceEdgesV1>,
    CanonicalProtectedCallableSourceInterfacesV1,
    Vec<ExportDefinitionSourceV1>,
);

pub(super) fn produce(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    public_nominals: &CanonicalNominalInterfacesV1,
    public_callables: &CanonicalCallableInterfacesV1,
    public_sources: &CanonicalCallableSourceInterfacesV1,
) -> Result<InheritanceProjection, Error> {
    let mut records = Vec::with_capacity(nominals.len());
    let mut edges = Vec::with_capacity(nominals.len());
    let mut origins = Vec::new();
    let mut sources = Vec::new();
    for nominal in nominals {
        let source_id = SourceNominalId::Concrete(nominal.owner);
        let public = public_nominals
            .get(source_id)
            .ok_or(Error::MissingLocalSupport(nominal.exact))?;
        let edge = project_edges(export, nominal)?;
        let authority_edge = project_edges(export, nominal)?;
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
        let schemas = schemas::project(export, nominal)?;
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
        edges.push(authority_edge);
        records.push(record);
    }
    edges.sort_unstable_by_key(NominalInheritanceEdgesV1::owner);
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
    Ok((table, edges, sources, origins))
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

fn project_edges(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<NominalInheritanceEdgesV1, Error> {
    let (modality, base, interfaces) = match nominal.local {
        NominalLocalId::Struct(id) => (
            NominalInheritanceModalityV1::Final,
            None,
            export.structs[id].interfaces.as_slice(),
        ),
        NominalLocalId::Enum(id) => (
            NominalInheritanceModalityV1::Final,
            None,
            export.enums[id].interfaces.as_slice(),
        ),
        NominalLocalId::Class(id) => {
            let class = &export.classes[id];
            let modality = match class.modifier {
                ClassModifier::Final => NominalInheritanceModalityV1::Final,
                ClassModifier::Open => NominalInheritanceModalityV1::Open,
                ClassModifier::Abstract => NominalInheritanceModalityV1::Abstract,
            };
            (modality, class.base_class, class.interfaces.as_slice())
        }
        NominalLocalId::Interface(id) => {
            let interface = &export.interfaces[id];
            let parents = interface
                .parents
                .iter()
                .map(|parent| export.interface_applications[*parent].canonical_type)
                .collect::<Vec<_>>();
            return build_edges(
                export,
                nominal.exact,
                NominalInheritanceModalityV1::Interface,
                None,
                &parents,
            );
        }
        NominalLocalId::Object(id) => {
            let class = &export.classes[export.objects[id].backing_class];
            (
                NominalInheritanceModalityV1::Final,
                class.base_class,
                class.interfaces.as_slice(),
            )
        }
    };
    build_edges(export, nominal.exact, modality, base, interfaces)
}

fn build_edges(
    export: &ExportHir,
    owner: PersistentExactTypeId,
    modality: NominalInheritanceModalityV1,
    base: Option<TypeId>,
    interfaces: &[TypeId],
) -> Result<NominalInheritanceEdgesV1, Error> {
    let base = match base {
        Some(ty) => DirectClassBaseV1::ClassBase {
            exact: exact(export, ty)?,
        },
        None => DirectClassBaseV1::NoClassBase,
    };
    let interfaces = interfaces
        .iter()
        .map(|ty| exact(export, *ty))
        .collect::<Result<Vec<_>, _>>()?;
    NominalInheritanceEdgesV1::try_new(owner, modality, base, interfaces).map_err(|error| {
        Error::InvalidInheritance {
            exact: owner,
            reason: error.to_string(),
        }
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

fn exact(export: &ExportHir, ty: TypeId) -> Result<PersistentExactTypeId, Error> {
    let exact = export
        .type_identities
        .get(ty)
        .and_then(HirTypeIdentity::exact)
        .map(|record| record.id())
        .ok_or(Error::MissingExactIdentity)?;
    let generic_application = match &export.types[ty] {
        Type::Struct(application) => !export.struct_applications[*application]
            .arguments
            .is_empty(),
        Type::Enum(application) => !export.enum_applications[*application].arguments.is_empty(),
        Type::Class(application) => !export.class_applications[*application].arguments.is_empty(),
        Type::Interface(application) => !export.interface_applications[*application]
            .arguments
            .is_empty(),
        _ => false,
    };
    if generic_application {
        return Err(Error::GenericOdrRequired(exact));
    }
    Ok(exact)
}
