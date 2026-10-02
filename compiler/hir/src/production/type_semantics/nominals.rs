use scoop_identity::{
    DefinitionOriginSubject, DefinitionOwnerAtom, ExactTypeKey, PersistentExactTypeId,
    PersistentTypeId, SourceDeclarationKey,
};

use super::{CrossConeTypeSemanticsProductionError as Error, *};
use crate::*;

mod representation;
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
    output: &DependencyHirOutput,
    metadata: SharedTypeMetadataV1<'_>,
    dependencies: &[SharedTypeMetadataV1<'_>],
) -> Result<CrossConeTypeSemanticsSectionV1, Error> {
    let export = output.output().export.module();
    let local = output.output().local.module();
    if metadata.provider != export.cone {
        return Err(Error::PublicInterface(
            "shared metadata has a different provider".into(),
        ));
    }
    let required = CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export)?;
    let materialization = NominalMaterializationClosure::from_declarations(
        metadata.public.nominal_interfaces(),
        metadata.public.callable_interfaces(),
    )
    .map_err(inheritance::source_errors::invalid)?;
    let concrete = source_inventory::from_required(output, &required, &materialization)?;
    let root_exacts = concrete.iter().map(|nominal| nominal.exact).collect();
    let fact_requirements = representation::fact_requirements(export, &concrete)?;
    let facts = facts::produce(local, &root_exacts, &fact_requirements)?;
    let mut representations = Vec::with_capacity(concrete.len());
    for nominal in &concrete {
        let source_id = SourceNominalId::Concrete(nominal.owner);
        let access = declaration_access(
            export,
            nominal.source.declaration(),
            nominal_access(export, nominal.local).declared.into(),
        )?;
        let shape = representation::shape(export, local, nominal)?;
        let record =
            NominalRepresentationSupportV1::try_new(nominal.source.declaration(), access, shape)
                .map_err(|error| Error::InvalidRepresentation {
                    declaration: source_id,
                    reason: error.to_string(),
                })?;
        representations.push(record);
    }

    let inheritance_inventory = inheritance::source_inventory(export, &concrete)?;
    let slot_selections = inheritance::slot_selections(metadata, &concrete)?;
    let slots = inheritance::SlotContracts::new(
        metadata,
        dependencies,
        &inheritance_inventory,
        &slot_selections,
    )?;
    let inheritance = inheritance::produce(export, &concrete, &inheritance_inventory, &slots)?;

    let representation_support = CanonicalNominalRepresentationSupportV1::try_new(representations)
        .map_err(|error| Error::InvalidTable {
            table: "representation-support",
            reason: error.to_string(),
        })?;
    let section = CrossConeTypeSemanticsSectionV1::new(
        facts,
        representation_support,
        inheritance,
        metadata
            .materialized_type_uses(dependencies)
            .map_err(|error| Error::SharedTypeMetadata(Box::new(error)))?,
    );
    Ok(section)
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

pub(super) fn source_id(source: &HirSourceNominalIdentity) -> SourceNominalId {
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
        .ok_or(Error::MissingExactIdentity {
            context: "source nominal type",
        })?
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

pub(super) fn all_nominals(export: &ExportHir) -> impl Iterator<Item = NominalLocalId> + '_ {
    export
        .structs
        .iter()
        .map(|(id, _)| NominalLocalId::Struct(id))
        .chain(export.enums.iter().map(|(id, _)| NominalLocalId::Enum(id)))
        .chain(
            export
                .classes
                .iter()
                .map(|(id, _)| NominalLocalId::Class(id)),
        )
        .chain(
            export
                .interfaces
                .iter()
                .map(|(id, _)| NominalLocalId::Interface(id)),
        )
        .chain(
            export
                .objects
                .iter()
                .map(|(id, _)| NominalLocalId::Object(id)),
        )
}
