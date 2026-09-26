use super::*;
use scoop_identity::{CanonicalIdentifier, SignatureTypeKey};

pub(super) fn project(
    export: &ExportHir,
    signatures: &HirInterfaceSignatureProjector<'_>,
    declaration: CallableTemplateOrigin,
    expected: &[SignatureTypeKey],
    interface: &ExportParameterInterface,
    binders: &[HirSignatureBinder],
) -> Result<ProtectedCallableSourceInterfaceV1, Error> {
    if interface.parameters.len() != expected.len() {
        return Err(invalid(
            "nominal parameter arity differs from its declaration",
        ));
    }
    let path = WirePath::root();

    let mut parameters = Vec::new();
    scoop_wire::allocation::try_reserve(&mut parameters, expected.len(), &path)
        .map_err(resource)?;
    for (position, (parameter, expected)) in interface.parameters.iter().zip(expected).enumerate() {
        let (ty, kind) = calling(export, parameter.calling)?;

        let value = signatures.map_type(ty, binders).map_err(invalid)?;
        if &value != expected {
            return Err(invalid(
                "nominal parameter type differs from its declaration",
            ));
        }
        let name = CanonicalIdentifier::new(&parameter.name).map_err(invalid)?;
        export
            .source_files
            .get(parameter.origin.file as usize)
            .ok_or_else(|| invalid("nominal parameter has no source file"))?;

        export
            .source_context_identities
            .get(parameter.origin.context)
            .ok_or_else(|| invalid("nominal parameter has no persistent source context"))?;

        let origin = crate::production::definition_sources::project_definition_source(
            export,
            parameter.origin,
        )
        .map_err(invalid)?;
        let position = u32::try_from(position).map_err(invalid)?;
        let calling = parameter_calling(declaration, position, kind, &value)?;
        parameters.push(ProtectedSourceParameterV1::new(
            name, value, calling, origin,
        ));
    }
    let parameters = CanonicalProtectedSourceParametersV1::try_new(parameters).map_err(invalid)?;
    ProtectedCallableSourceInterfaceV1::try_new(declaration, parameters).map_err(invalid)
}

fn calling(
    export: &ExportHir,
    calling: ExportParameterCalling,
) -> Result<(TypeId, ProtectedParameterCallingKindV1), Error> {
    use ProtectedParameterCallingKindV1 as Kind;
    Ok(match calling {
        ExportParameterCalling::Required { value_type } => (value_type, Kind::Required),
        ExportParameterCalling::Default { value_type, source } => {
            require_default(export, source)?;
            (value_type, Kind::Default)
        }
        ExportParameterCalling::Vararg {
            parameter_type,
            omission,
        } => {
            if parameter_type.into_raw().into_u32() as usize
                >= export.export_vararg_parameter_types.len()
            {
                return Err(invalid("nominal parameter has no sealed vararg type"));
            }
            let parameter = &export.export_vararg_parameter_types[parameter_type];
            let kind = match omission {
                ExportVarargOmission::EmptyArray => Kind::VarargEmpty,
                ExportVarargOmission::Default(source) => {
                    require_default(export, source)?;
                    Kind::VarargDefault
                }
            };
            (parameter.array_type, kind)
        }
    })
}
fn require_default(export: &ExportHir, source: ExportDefaultSourceId) -> Result<(), Error> {
    let index: u32 = source.into_raw().into();
    if index as usize >= export.export_default_sources.len() {
        Err(invalid("nominal parameter has no sealed default source"))
    } else {
        Ok(())
    }
}

fn parameter_calling(
    owner: CallableTemplateOrigin,
    position: u32,
    kind: ProtectedParameterCallingKindV1,
    value_type: &SignatureTypeKey,
) -> Result<ProtectedParameterCallingV1, Error> {
    use ProtectedParameterCallingKindV1 as Kind;
    use ProtectedParameterCallingV1 as Calling;
    let template = || ProtectedDefaultTemplateKeyV1::try_new(owner, position).map_err(invalid);
    match kind {
        Kind::Required => Ok(Calling::Required),
        Kind::Default => Ok(Calling::Default {
            template: template()?,
        }),
        Kind::VarargEmpty | Kind::VarargDefault => {
            let SignatureTypeKey::NominalApplication { arguments, .. } = value_type else {
                return Err(invalid("source vararg value has no Array application"));
            };
            let [element] = arguments.as_slice() else {
                return Err(invalid("source vararg Array must have one type argument"));
            };

            let element_type = element.clone();
            if kind == Kind::VarargEmpty {
                Ok(Calling::VarargEmpty { element_type })
            } else {
                Ok(Calling::VarargDefault {
                    element_type,
                    template: template()?,
                })
            }
        }
    }
}
