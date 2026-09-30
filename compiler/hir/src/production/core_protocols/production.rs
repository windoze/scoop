use super::*;

impl CoreProtocolCallableV1 {
    pub(in crate::production) fn from_function(
        export: &ExportHir,
        protocols: &crate::DefinedCoreProtocols,
        function: FunctionId,
    ) -> Result<Self, CoreProtocolCallableBuildError> {
        let declaration = arena_get(&export.functions, function).ok_or(
            CoreProtocolCallableBuildError::UnknownFunction(raw_index(function)),
        )?;
        let source = match &export.function_identities[function] {
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
                CoreProtocolCallableDefinitionV1::Function(record.id())
            }
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(record)) => {
                CoreProtocolCallableDefinitionV1::GenericFunction(record.id())
            }
            _ => {
                return Err(CoreProtocolCallableBuildError::NonSourceFunction(
                    raw_index(function),
                ));
            }
        };
        require_definition_origin(export, source)?;

        let (owner_parameters, own_parameters): (
            Vec<&crate::TypeParamDecl>,
            Vec<&crate::TypeParamDecl>,
        ) = match &declaration.genericity {
            FunctionGenericity::Plain => (Vec::new(), Vec::new()),
            FunctionGenericity::Generic { parameters, .. } => {
                (Vec::new(), parameters.iter().collect())
            }
            FunctionGenericity::OwnerParameterizedMethod {
                owner_parameters, ..
            } => (owner_parameters.iter().collect(), Vec::new()),
            FunctionGenericity::GenericMethod {
                owner_parameters,
                method_parameters,
                ..
            } => (
                owner_parameters.iter().collect(),
                method_parameters.iter().collect(),
            ),
        };
        let own_parameter_count = u32::try_from(own_parameters.len()).map_err(|_| {
            CoreProtocolCallableBuildError::TooManyTypeParameters { definition: source }
        })?;
        let binders = function_signature_binders(&owner_parameters, &own_parameters, source)?;
        let source_key = match &export.function_identities[function] {
            HirFunctionIdentity::Source(source) => source.declaration(),
            _ => unreachable!("source identity was checked above"),
        };
        if source_key.duplicate_signature().type_parameter_count() != own_parameter_count {
            return Err(CoreProtocolCallableBuildError::TypeParameterCountMismatch {
                definition: source,
            });
        }
        let receiver = if source_key.duplicate_signature().receiver_is_present() {
            Some(
                declaration
                    .params
                    .first()
                    .ok_or(CoreProtocolCallableBuildError::MissingReceiver { definition: source })?
                    .ty,
            )
        } else {
            None
        };
        let parameter_types =
            source_parameter_types(export, ExportParameterOwner::Function(function), source)?;
        let mapper = HirSignatureTypeMapper::new(type_inputs(export, protocols));
        let receiver_shape = receiver
            .map(|ty| mapper.map(ty, &binders))
            .transpose()
            .map_err(
                |error| CoreProtocolCallableBuildError::InvalidSignatureType {
                    definition: source,
                    error,
                },
            )?;
        let parameter_shapes = map_types(&mapper, &parameter_types, &binders, source)?;
        validate_function_duplicate(
            source_key.duplicate_signature(),
            receiver_shape.as_ref(),
            &parameter_shapes,
        )
        .map_err(
            |()| CoreProtocolCallableBuildError::DuplicateSignatureMismatch { definition: source },
        )?;
        let result = mapper
            .map(declaration.return_ty, &binders)
            .map_err(
                |error| CoreProtocolCallableBuildError::InvalidSignatureType {
                    definition: source,
                    error,
                },
            )?;
        Ok(Self {
            definition: source,
            signature: SignatureCallableShape::new(
                function_effect(declaration.is_suspend),
                receiver_shape,
                parameter_shapes,
                result,
            ),
        })
    }

    pub(in crate::production) fn from_class_constructor(
        export: &ExportHir,
        protocols: &crate::DefinedCoreProtocols,
        constructor: ClassConstructorId,
    ) -> Result<Self, CoreProtocolCallableBuildError> {
        let declaration = arena_get(&export.class_constructors, constructor).ok_or(
            CoreProtocolCallableBuildError::UnknownClassConstructor(raw_index(constructor)),
        )?;
        let identity = &export.constructor_identities[constructor];
        let definition = match identity {
            HirClassConstructorIdentity::Source(record) => {
                CoreProtocolCallableDefinitionV1::Constructor(record.id())
            }
            HirClassConstructorIdentity::ZeroArgumentAdapter { record, .. } => {
                CoreProtocolCallableDefinitionV1::GeneratedCallable(record.id())
            }
        };
        require_definition_origin(export, definition)?;

        let owner = arena_get(&export.classes, declaration.owner).ok_or(
            CoreProtocolCallableBuildError::UnknownConstructorOwner {
                definition,
                owner: raw_index(declaration.owner),
            },
        )?;
        let binders = signature_binders(
            owner.type_params.iter().map(|parameter| parameter.id),
            definition,
        )?;
        let parameter_types = match identity {
            HirClassConstructorIdentity::Source(record) => {
                let parameters = source_parameter_types(
                    export,
                    ExportParameterOwner::ClassConstructor(constructor),
                    definition,
                )?;
                let shapes = map_types(
                    &HirSignatureTypeMapper::new(type_inputs(export, protocols)),
                    &parameters,
                    &binders,
                    definition,
                )?;
                validate_constructor_duplicate(record.key().duplicate_signature(), &shapes)
                    .map_err(
                        |()| CoreProtocolCallableBuildError::DuplicateSignatureMismatch {
                            definition,
                        },
                    )?;
                shapes
            }
            HirClassConstructorIdentity::ZeroArgumentAdapter { .. } => Vec::new(),
        };
        let owner_type = arena_get(&export.class_applications, owner.self_application)
            .ok_or(
                CoreProtocolCallableBuildError::UnknownConstructorSelfApplication { definition },
            )?
            .canonical_type;
        let mapper = HirSignatureTypeMapper::new(type_inputs(export, protocols));
        let result = mapper.map(owner_type, &binders).map_err(|error| {
            CoreProtocolCallableBuildError::InvalidSignatureType { definition, error }
        })?;
        Ok(Self {
            definition,
            signature: SignatureCallableShape::new(Effect::Ordinary, None, parameter_types, result),
        })
    }
}

fn require_definition_origin(
    export: &ExportHir,
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<(), CoreProtocolCallableBuildError> {
    if export
        .export_definition_origins
        .get(definition.origin_subject())
        .is_none()
    {
        return Err(CoreProtocolCallableBuildError::MissingDefinitionOrigin(
            definition,
        ));
    }
    Ok(())
}

fn signature_binders(
    parameters: impl Iterator<Item = crate::TypeParamId>,
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<Vec<HirSignatureBinder>, CoreProtocolCallableBuildError> {
    parameters
        .enumerate()
        .map(|(index, parameter)| {
            u32::try_from(index)
                .map(|index| HirSignatureBinder {
                    parameter,
                    depth: 0,
                    index,
                })
                .map_err(|_| CoreProtocolCallableBuildError::TooManyTypeParameters { definition })
        })
        .collect()
}

fn function_signature_binders(
    owner_parameters: &[&crate::TypeParamDecl],
    own_parameters: &[&crate::TypeParamDecl],
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<Vec<HirSignatureBinder>, CoreProtocolCallableBuildError> {
    let owner_depth = u32::from(!own_parameters.is_empty());
    let mut binders = owner_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            u32::try_from(index)
                .map(|index| HirSignatureBinder {
                    parameter: parameter.id,
                    depth: owner_depth,
                    index,
                })
                .map_err(|_| CoreProtocolCallableBuildError::TooManyTypeParameters { definition })
        })
        .collect::<Result<Vec<_>, _>>()?;
    binders.extend(signature_binders(
        own_parameters.iter().map(|parameter| parameter.id),
        definition,
    )?);
    Ok(binders)
}

fn source_parameter_types(
    export: &ExportHir,
    owner: ExportParameterOwner,
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<Vec<TypeId>, CoreProtocolCallableBuildError> {
    let mut interfaces = export
        .source_parameter_interfaces
        .iter()
        .filter(|interface| interface.owner == owner);
    let interface = interfaces
        .next()
        .ok_or(CoreProtocolCallableBuildError::MissingSourceParameterInterface { definition })?;
    if interfaces.next().is_some() {
        return Err(
            CoreProtocolCallableBuildError::DuplicateSourceParameterInterface { definition },
        );
    }
    interface
        .parameters
        .iter()
        .map(|parameter| match parameter.calling {
            ExportParameterCalling::Required { value_type }
            | ExportParameterCalling::Default { value_type, .. } => Ok(value_type),
            ExportParameterCalling::Vararg { parameter_type, .. } => {
                arena_get(&export.export_vararg_parameter_types, parameter_type)
                    .map(|parameter| parameter.array_type)
                    .ok_or(
                        CoreProtocolCallableBuildError::InvalidSourceParameterInterface {
                            definition,
                        },
                    )
            }
        })
        .collect()
}

fn map_types(
    mapper: &HirSignatureTypeMapper<'_>,
    types: &[TypeId],
    binders: &[HirSignatureBinder],
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<Vec<SignatureTypeKey>, CoreProtocolCallableBuildError> {
    types
        .iter()
        .map(|&ty| {
            mapper.map(ty, binders).map_err(|error| {
                CoreProtocolCallableBuildError::InvalidSignatureType { definition, error }
            })
        })
        .collect()
}

fn type_inputs<'a>(
    export: &'a ExportHir,
    protocols: &'a crate::DefinedCoreProtocols,
) -> HirTypeIdentityInputs<'a> {
    HirTypeIdentityInputs {
        types: &export.types,
        function_types: &export.function_types,
        structs: &export.structs,
        struct_applications: &export.struct_applications,
        enums: &export.enums,
        loaded_enum_definitions: &export.loaded_enum_definitions,
        loaded_struct_definitions: &export.loaded_struct_definitions,
        enum_applications: &export.enum_applications,
        classes: &export.classes,
        class_applications: &export.class_applications,
        interfaces: &export.interfaces,
        interface_applications: &export.interface_applications,
        objects: &export.objects,
        core_types: crate::HirCoreTypeIdentityAuthority::Defined(&protocols.fundamental_types),
        nominal_identities: &export.nominal_identities,
    }
}

fn validate_function_duplicate(
    duplicate: &DuplicateSignatureKey,
    receiver: Option<&SignatureTypeKey>,
    parameters: &[SignatureTypeKey],
) -> Result<(), ()> {
    let DuplicateSignatureKey::Function {
        receiver: expected_receiver,
        parameters: expected_parameters,
        ..
    } = duplicate
    else {
        return Err(());
    };
    let receiver_matches = match (receiver, expected_receiver) {
        (None, OptionalSignatureType::Absent) => true,
        (Some(actual), OptionalSignatureType::Present(expected)) => actual == expected.as_ref(),
        _ => false,
    };
    (receiver_matches && parameters == expected_parameters)
        .then_some(())
        .ok_or(())
}

fn validate_constructor_duplicate(
    duplicate: &DuplicateSignatureKey,
    parameters: &[SignatureTypeKey],
) -> Result<(), ()> {
    matches!(
        duplicate,
        DuplicateSignatureKey::Constructor {
            parameters: expected
        } if expected == parameters
    )
    .then_some(())
    .ok_or(())
}

const fn function_effect(is_suspend: bool) -> Effect {
    if is_suspend {
        Effect::Suspend
    } else {
        Effect::Ordinary
    }
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}
