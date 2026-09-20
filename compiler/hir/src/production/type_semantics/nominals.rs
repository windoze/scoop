use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    DefinitionOriginSubject, DefinitionOwnerAtom, ExactTypeKey, PersistentExactTypeId,
    PersistentTypeId, SourceDeclarationKey,
};

use super::{CrossConeTypeSemanticsProductionError as Error, *};
use crate::*;

mod authority_projection;
mod representation;
mod source_foundation;
mod source_inventory;

#[derive(Clone, Copy)]
pub(super) enum NominalLocalId {
    Struct(StructId),
    Enum(EnumId),
    Class(ClassId),
    Interface(InterfaceId),
    Object(ObjectId),
}

pub(super) struct ConcreteNominal<'a> {
    pub local: NominalLocalId,
    pub source: &'a HirSourceNominalIdentity,
    pub owner: PersistentTypeId,
    pub exact: PersistentExactTypeId,
}

pub(super) fn produce(
    output: &OrdinaryHirOutput<'_>,
    public: &CrossConeHirInterfaceSectionV1,
    meter: &mut scoop_wire::BudgetMeter,
) -> Result<CrossConeTypeSemanticsProductionV1, Error> {
    let export = output.output().export.module();
    let local = output.output().local.module();
    let source_foundation::Projection {
        concrete,
        root_exacts,
        public: projected_public,
        foundation,
    } = source_foundation::project(output)?;
    if &projected_public != public.nominal_interfaces() {
        return Err(Error::PublicInterface(
            "the supplied M23-5 section was not projected from this Export HIR".into(),
        ));
    }
    for local_id in public_nominals(export) {
        let identity = identity(export, local_id)?;
        let source = identity.source().ok_or_else(|| {
            let (kind, index) = location(local_id);
            Error::GeneratedPublicNominal { kind, index }
        })?;
        inheritance::reject_unsupported_source_features(export, local_id, source_id(source))?;
    }
    reject_protected_nominals(export, &foundation.source_roots().iter().copied().collect())?;
    meter
        .charge_collection_slots(
            foundation.source_roots().len() as u64,
            &scoop_wire::WirePath::root(),
        )
        .map_err(|error| Error::SourceInventory(SourceInventoryError::Resource(error)))?;
    let required_nominals =
        CanonicalSourceNominalIdsV1::try_new(foundation.source_roots().to_vec(), meter)
            .map_err(Error::SourceInventory)?;
    let source_nominals = CanonicalNominalSourceContractsV1::from_export_hir(
        &output.output().export,
        &required_nominals,
        meter,
    )?;
    let fact_requirements = representation::fact_requirements(export, &concrete)?;
    let facts = facts::candidate(
        output.imported_core(),
        export,
        local,
        &root_exacts,
        &fact_requirements,
    )?;
    let mut representations = Vec::with_capacity(concrete.len());
    let mut definition_sources = Vec::new();
    for nominal in &concrete {
        let source_id = SourceNominalId::Concrete(nominal.owner);
        let access = declaration_access(
            export,
            nominal.source.declaration(),
            nominal_access(export, nominal.local).declared.into(),
        )?;
        definition_sources.push(access.definition_origin().clone());
        let shape = representation::shape(export, local, nominal)?;
        let record =
            NominalRepresentationSupportV1::try_new(nominal.source.declaration(), access, shape)
                .map_err(|error| Error::InvalidRepresentation {
                    declaration: source_id,
                    reason: error.to_string(),
                })?;
        representations.push(record);
    }

    let public_callables = CanonicalCallableInterfacesV1::from_export_hir(export)
        .map_err(|error| Error::PublicInterface(error.to_string()))?;
    if &public_callables != public.callable_interfaces() {
        return Err(Error::PublicInterface(
            "the supplied callable table was not projected from this Export HIR".into(),
        ));
    }
    let public_sources = CanonicalCallableSourceInterfacesV1::from_export_hir(export)
        .map_err(|error| Error::PublicInterface(error.to_string()))?;
    if &public_sources != public.source_interfaces() {
        return Err(Error::PublicInterface(
            "the supplied source-call table was not projected from this Export HIR".into(),
        ));
    }
    let inheritance_inventory = inheritance::source_inventory(export, &concrete, meter)?;
    let interface_sources = inheritance::interface_sources(export, &concrete, meter)?;
    let slot_selections = inheritance::slot_selections(export, &concrete, meter)?;
    let source_callables =
        inheritance::source_callables(export, &inheritance_inventory, &slot_selections, meter)?;
    let source_constructors =
        inheritance::source_constructors(export, &concrete, &inheritance_inventory, meter)?;
    let source_protected_callables =
        inheritance::source_protected_callables(export, &inheritance_inventory, meter)?;
    let source_properties =
        inheritance::source_properties(export, &inheritance_inventory, &slot_selections, meter)?;
    let source_parameters = inheritance::source_parameters(export, &inheritance_inventory, meter)?;
    let (inheritance, protected_sources, constructor_origins) = inheritance::produce(
        export,
        &concrete,
        &projected_public,
        &public_callables,
        &public_sources,
        meter,
    )?;
    definition_sources.extend(constructor_origins);

    let representation_support = CanonicalNominalRepresentationSupportV1::try_new(representations)
        .map_err(|error| Error::InvalidTable {
            table: "representation-support",
            reason: error.to_string(),
        })?;
    let definition_source_evidence = definition_sources.into_iter().collect::<BTreeSet<_>>();
    let definition_sources = CanonicalExportDefinitionSourcesV1::try_new(
        definition_source_evidence.iter().cloned().collect(),
    )
    .map_err(|error| Error::InvalidTable {
        table: "definition-source",
        reason: error.to_string(),
    })?;
    let section = CrossConeTypeSemanticsSectionV1::new(
        facts,
        representation_support,
        inheritance,
        CanonicalProtectedDeclarationInterfacesV1::try_new(Vec::new()).map_err(|error| {
            Error::InvalidTable {
                table: "protected-declaration",
                reason: error.to_string(),
            }
        })?,
        protected_sources,
        CanonicalProtectedDefaultTemplatesV1::try_new(Vec::new()).map_err(|error| {
            Error::InvalidTable {
                table: "protected-default",
                reason: error.to_string(),
            }
        })?,
        definition_sources,
        CanonicalSelectedExternalTypeUsesV1::try_new(Vec::new()).map_err(|error| {
            Error::InvalidTable {
                table: "selected-external-type-use",
                reason: error.to_string(),
            }
        })?,
    );
    Ok(CrossConeTypeSemanticsProductionV1 {
        section,
        foundation,
        inheritance_inventory,
        interface_sources,
        slot_selections,
        source_callables,
        source_constructors,
        source_protected_callables,
        source_properties,
        source_parameters,
        source_nominals,
    })
}

fn reject_protected_nominals(
    export: &ExportHir,
    source_roots: &BTreeSet<SourceNominalId>,
) -> Result<(), Error> {
    for (local, access) in export
        .structs
        .iter()
        .map(|(id, declaration)| (NominalLocalId::Struct(id), &declaration.access))
        .chain(
            export
                .enums
                .iter()
                .map(|(id, declaration)| (NominalLocalId::Enum(id), &declaration.access)),
        )
        .chain(
            export
                .classes
                .iter()
                .map(|(id, declaration)| (NominalLocalId::Class(id), &declaration.access)),
        )
        .chain(
            export
                .interfaces
                .iter()
                .map(|(id, declaration)| (NominalLocalId::Interface(id), &declaration.access)),
        )
        .chain(
            export
                .objects
                .iter()
                .map(|(id, declaration)| (NominalLocalId::Object(id), &declaration.access)),
        )
    {
        if access.declared == DeclaredVisibility::Protected {
            let source = identity(export, local)?
                .source()
                .ok_or(Error::MissingExactIdentity)?;
            if !lexical_owners(source.declaration())?
                .iter()
                .any(|owner| source_roots.contains(owner))
            {
                continue;
            }
            return Err(Error::UnsupportedProtectedNominal(source_id(source)));
        }
    }
    Ok(())
}

pub(super) fn nominal_access(export: &ExportHir, local: NominalLocalId) -> &NominalAccess {
    match local {
        NominalLocalId::Struct(id) => &export.structs[id].access,
        NominalLocalId::Enum(id) => &export.enums[id].access,
        NominalLocalId::Class(id) => &export.classes[id].access,
        NominalLocalId::Interface(id) => &export.interfaces[id].access,
        NominalLocalId::Object(id) => &export.objects[id].access,
    }
}

fn public_nominals(export: &ExportHir) -> impl Iterator<Item = NominalLocalId> + '_ {
    export
        .public_surface
        .structs
        .iter()
        .copied()
        .map(NominalLocalId::Struct)
        .chain(
            export
                .public_surface
                .enums
                .iter()
                .copied()
                .map(NominalLocalId::Enum),
        )
        .chain(
            export
                .public_surface
                .classes
                .iter()
                .copied()
                .map(NominalLocalId::Class),
        )
        .chain(
            export
                .public_surface
                .interfaces
                .iter()
                .copied()
                .map(NominalLocalId::Interface),
        )
        .chain(
            export
                .public_surface
                .objects
                .iter()
                .copied()
                .map(NominalLocalId::Object),
        )
}

pub(super) fn identity(
    export: &ExportHir,
    local: NominalLocalId,
) -> Result<&HirNominalIdentity, Error> {
    let identity = match local {
        NominalLocalId::Struct(id) => export.nominal_identities.get_struct(id),
        NominalLocalId::Enum(id) => export.nominal_identities.get_enum(id),
        NominalLocalId::Class(id) => export.nominal_identities.get_class(id),
        NominalLocalId::Interface(id) => export.nominal_identities.get_interface(id),
        NominalLocalId::Object(id) => export.nominal_identities.get_object(id),
    };
    identity.ok_or_else(|| {
        let (kind, index) = location(local);
        Error::MissingNominalIdentity { kind, index }
    })
}

fn location(local: NominalLocalId) -> (TypeSemanticsNominalKind, u32) {
    match local {
        NominalLocalId::Struct(id) => (TypeSemanticsNominalKind::Struct, raw(id)),
        NominalLocalId::Enum(id) => (TypeSemanticsNominalKind::Enum, raw(id)),
        NominalLocalId::Class(id) => (TypeSemanticsNominalKind::Class, raw(id)),
        NominalLocalId::Interface(id) => (TypeSemanticsNominalKind::Interface, raw(id)),
        NominalLocalId::Object(id) => (TypeSemanticsNominalKind::Object, raw(id)),
    }
}

fn raw<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

fn source_id(source: &HirSourceNominalIdentity) -> SourceNominalId {
    match source {
        HirSourceNominalIdentity::Concrete(record) => SourceNominalId::Concrete(record.id()),
        HirSourceNominalIdentity::Generic(record) => SourceNominalId::GenericTemplate(record.id()),
    }
}

fn verify_exact_pair(
    export: &ExportHir,
    local: &LocalConcreteHir,
    nominal: NominalLocalId,
    exact: PersistentExactTypeId,
) -> Result<(), Error> {
    let export_type = match nominal {
        NominalLocalId::Struct(id) => {
            export.struct_applications[export.structs[id].self_application].canonical_type
        }
        NominalLocalId::Enum(id) => {
            export.enum_applications[export.enums[id].self_application].canonical_type
        }
        NominalLocalId::Class(id) => {
            export.class_applications[export.classes[id].self_application].canonical_type
        }
        NominalLocalId::Interface(id) => {
            export.interface_applications[export.interfaces[id].self_application].canonical_type
        }
        NominalLocalId::Object(id) => {
            export.object_types[export.objects[id].object_type].canonical_type
        }
    };
    let export_exact = export.type_identities[export_type]
        .exact()
        .ok_or(Error::MissingExactIdentity)?
        .id();
    if export_exact != exact
        || local
            .exact_type_identities
            .type_for_identity(exact)
            .is_none()
    {
        return Err(Error::ExactIdentityMismatch(exact));
    }
    Ok(())
}

pub(super) fn declaration_access(
    export: &ExportHir,
    key: &SourceDeclarationKey,
    visibility: DeclaredVisibilityV1,
) -> Result<DeclarationAccessSourceV1, Error> {
    let declaration = source_id_from_key(key)?;
    let owner = match key.duplicate_signature().type_parameter_count() {
        0 => {
            DefinitionOriginSubject::Type(PersistentTypeId::from_source_declaration(key).map_err(
                |error| Error::InvalidSourceShape {
                    declaration,
                    reason: error.to_string(),
                },
            )?)
        }
        _ => DefinitionOriginSubject::GenericType(
            scoop_identity::PersistentGenericTypeId::from_source_declaration(key).map_err(
                |error| Error::InvalidSourceShape {
                    declaration,
                    reason: error.to_string(),
                },
            )?,
        ),
    };
    declaration_access_for_subject(export, key, owner, visibility)
}

pub(super) fn declaration_access_for_subject(
    export: &ExportHir,
    key: &SourceDeclarationKey,
    subject: DefinitionOriginSubject,
    visibility: DeclaredVisibilityV1,
) -> Result<DeclarationAccessSourceV1, Error> {
    let origin = export
        .export_definition_origins
        .get(subject)
        .ok_or(Error::MissingDefinitionOrigin(subject))?;
    DeclarationAccessSourceV1::try_new(
        visibility,
        lexical_owners(key)?,
        ExportDefinitionSourceV1::new(origin.origin().clone()),
    )
    .map_err(|error| Error::InvalidSourceDeclaration(error.to_string()))
}

fn source_id_from_key(key: &SourceDeclarationKey) -> Result<SourceNominalId, Error> {
    if key.duplicate_signature().type_parameter_count() == 0 {
        PersistentTypeId::from_source_declaration(key)
            .map(SourceNominalId::Concrete)
            .map_err(|error| Error::InvalidSourceDeclaration(error.to_string()))
    } else {
        scoop_identity::PersistentGenericTypeId::from_source_declaration(key)
            .map(SourceNominalId::GenericTemplate)
            .map_err(|error| Error::InvalidSourceDeclaration(error.to_string()))
    }
}

pub(super) fn lexical_owners(key: &SourceDeclarationKey) -> Result<Vec<SourceNominalId>, Error> {
    key.owners()
        .owners()
        .iter()
        .map(|owner| match owner {
            DefinitionOwnerAtom::Type(id) => Ok(SourceNominalId::Concrete(*id)),
            DefinitionOwnerAtom::GenericType(id) => Ok(SourceNominalId::GenericTemplate(*id)),
            other => Err(Error::InvalidLexicalOwner(other.clone())),
        })
        .collect()
}
