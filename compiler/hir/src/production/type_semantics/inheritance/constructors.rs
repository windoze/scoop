use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, PersistentConstructorId};

use super::super::CrossConeTypeSemanticsProductionError as Error;
use super::super::nominals::{ConcreteNominal, NominalLocalId, declaration_access_for_subject};
use crate::*;

pub(super) fn project(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
    public_nominal: &NominalInterfaceRecordV1,
    public_callables: &CanonicalCallableInterfacesV1,
    public_sources: &CanonicalCallableSourceInterfacesV1,
    sources: &mut Vec<ProtectedCallableSourceInterfaceV1>,
    origins: &mut Vec<ExportDefinitionSourceV1>,
) -> Result<CanonicalInheritanceConstructorsV1, Error> {
    let mut constructors = Vec::new();
    for declaration in public_nominal.constructors().values() {
        let callable = public_callables
            .get(CallableTemplateOrigin::Constructor(*declaration))
            .ok_or(Error::MissingConstructor(nominal.exact))?;
        if callable.owner().nominal_owner() != Some(SourceNominalId::Concrete(nominal.owner))
            || callable.receiver().is_some()
        {
            return Err(Error::MissingConstructor(nominal.exact));
        }
        let (key, visibility) =
            source(export, *declaration).ok_or(Error::MissingConstructor(nominal.exact))?;
        let access = declaration_access_for_subject(
            export,
            key,
            DefinitionOriginSubject::Constructor(*declaration),
            visibility.into(),
        )?;
        origins.push(access.definition_origin().clone());
        let payload = NominalSourceCallablePayloadV1::try_new(
            CallableTemplateOrigin::Constructor(*declaration),
            SourceNominalId::Concrete(nominal.owner),
            callable.type_parameters().clone(),
            callable.parameters().clone(),
            callable.result().clone(),
            callable.effects(),
            callable.modality(),
            CanonicalProtectedSlotRefsV1::try_new(Vec::new()).map_err(|error| {
                Error::InvalidInheritance {
                    exact: nominal.exact,
                    reason: error.to_string(),
                }
            })?,
        )
        .map_err(|error| Error::InvalidInheritance {
            exact: nominal.exact,
            reason: error.to_string(),
        })?;
        let source = NominalSupportConstructorInterfaceV1::try_new(*declaration, access, payload)
            .map_err(|error| Error::InvalidInheritance {
            exact: nominal.exact,
            reason: error.to_string(),
        })?;
        constructors.push(
            InheritanceConstructorInterfaceV1::try_new(source).map_err(|error| {
                Error::InvalidInheritance {
                    exact: nominal.exact,
                    reason: error.to_string(),
                }
            })?,
        );
        sources.push(project_source_protocol(
            CallableTemplateOrigin::Constructor(*declaration),
            public_sources,
            origins,
        )?);
    }
    CanonicalInheritanceConstructorsV1::try_new(constructors).map_err(|error| {
        Error::InvalidInheritance {
            exact: nominal.exact,
            reason: error.to_string(),
        }
    })
}

fn project_source_protocol(
    owner: CallableTemplateOrigin,
    public_sources: &CanonicalCallableSourceInterfacesV1,
    origins: &mut Vec<ExportDefinitionSourceV1>,
) -> Result<ProtectedCallableSourceInterfaceV1, Error> {
    let source = public_sources
        .get(owner)
        .ok_or(Error::MissingSourceInterface(owner))?;
    let mut parameters = Vec::with_capacity(source.parameters().parameters().len());
    for parameter in source.parameters().parameters() {
        let calling = match parameter.calling() {
            CallableParameterCallingV1::Required => ProtectedParameterCallingV1::Required,
            CallableParameterCallingV1::VarargEmpty { element_type } => {
                ProtectedParameterCallingV1::VarargEmpty {
                    element_type: element_type.clone(),
                }
            }
            CallableParameterCallingV1::Default { .. }
            | CallableParameterCallingV1::VarargDefault { .. } => {
                return Err(Error::DefaultTemplateAuthorityRequired(owner));
            }
        };
        origins.push(parameter.definition_origin().clone());
        parameters.push(ProtectedSourceParameterV1::new(
            parameter.name().clone(),
            parameter.value_type().clone(),
            calling,
            parameter.definition_origin().clone(),
        ));
    }
    let parameters =
        CanonicalProtectedSourceParametersV1::try_new(parameters).map_err(|error| {
            Error::InvalidTable {
                table: "protected-source-parameter",
                reason: error.to_string(),
            }
        })?;
    ProtectedCallableSourceInterfaceV1::try_new(owner, parameters).map_err(|error| {
        Error::InvalidTable {
            table: "protected-source-interface",
            reason: error.to_string(),
        }
    })
}

pub(super) fn has_protected(export: &ExportHir, local: NominalLocalId) -> bool {
    match local {
        NominalLocalId::Struct(id) => export.structs[id].constructors.iter().any(|id| {
            export.struct_constructors[*id].access.declared == DeclaredVisibility::Protected
        }),
        NominalLocalId::Class(id) => export.classes[id].constructors.iter().any(|id| {
            export.class_constructors[*id].access.declared == DeclaredVisibility::Protected
        }),
        NominalLocalId::Enum(_) | NominalLocalId::Interface(_) | NominalLocalId::Object(_) => false,
    }
}

fn source(
    export: &ExportHir,
    target: PersistentConstructorId,
) -> Option<(&scoop_identity::SourceDeclarationKey, DeclaredVisibility)> {
    for (id, constructor) in export.struct_constructors.iter() {
        let identity = &export.constructor_identities[id];
        if identity.id() == target {
            return Some((identity.key(), constructor.access.declared));
        }
    }
    for (id, constructor) in export.class_constructors.iter() {
        let Some(identity) = export.constructor_identities[id].source_record() else {
            continue;
        };
        if identity.id() == target {
            return Some((identity.key(), constructor.access.declared));
        }
    }
    None
}
