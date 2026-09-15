//! Arena-independent callable contracts used by the trusted core protocol surface.

use std::fmt;

use la_arena::{Arena, Idx};
use scoop_identity::{
    DecodedPersistentId, DecodedSignatureCallableShape, DefinitionOriginSubject,
    DefinitionOwnerAtom, DuplicateSignatureKey, Effect, GeneratedCallableKey,
    OptionalSignatureType, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentTypeId, SignatureCallableShape, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CanonicalHirFoundation, ClassConstructorId, ExportHir, ExportParameterCalling,
    ExportParameterOwner, FunctionGenericity, FunctionId, HirClassConstructorIdentity,
    HirFunctionIdentity, HirSignatureBinder, HirSignatureTypeMapper, HirSourceFunctionIdentity,
    HirTypeIdentityInputs, TypeId,
};
use scoop_identity::ConeIdentity;

/// Kind-specific persistent identity of a callable named by a compiler
/// protocol. Generic templates, source constructors, and generated adapters
/// remain distinct entities all the way through the artifact.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreProtocolCallableDefinitionV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    GeneratedCallable(PersistentGeneratedCallableId),
}

impl CoreProtocolCallableDefinitionV1 {
    const fn origin_subject(self) -> DefinitionOriginSubject {
        match self {
            Self::Function(id) => DefinitionOriginSubject::Function(id),
            Self::GenericFunction(id) => DefinitionOriginSubject::GenericFunction(id),
            Self::Constructor(id) => DefinitionOriginSubject::Constructor(id),
            Self::GeneratedCallable(id) => DefinitionOriginSubject::GeneratedCallable(id),
        }
    }
}

impl WireEncode for CoreProtocolCallableDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
            Self::Constructor(id) => encode_value_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_value_sum(encoder, 4, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCoreProtocolCallableDefinitionV1 {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    GeneratedCallable(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl WireEncode for DecodedCoreProtocolCallableDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
            Self::Constructor(id) => encode_value_sum(encoder, 3, id),
            Self::GeneratedCallable(id) => encode_value_sum(encoder, 4, id),
        }
    }
}

impl WireDecode for DecodedCoreProtocolCallableDefinitionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        require_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GeneratedCallable),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

/// Complete source-level signature of one compiler protocol callable. The
/// signature is repeated here because internal core callables are not part of
/// the direct-public target surface.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CoreProtocolCallableV1 {
    definition: CoreProtocolCallableDefinitionV1,
    signature: SignatureCallableShape,
}

impl CoreProtocolCallableV1 {
    pub const fn definition(&self) -> CoreProtocolCallableDefinitionV1 {
        self.definition
    }

    pub const fn signature(&self) -> &SignatureCallableShape {
        &self.signature
    }

    #[cfg(test)]
    pub(crate) const fn for_test(
        definition: CoreProtocolCallableDefinitionV1,
        signature: SignatureCallableShape,
    ) -> Self {
        Self {
            definition,
            signature,
        }
    }

    pub(super) fn from_function(
        export: &ExportHir,
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
        require_core_origin(export, source)?;

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
        let mapper = HirSignatureTypeMapper::new(type_inputs(export));
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

    pub(super) fn from_class_constructor(
        export: &ExportHir,
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
        require_core_origin(export, definition)?;

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
                    &HirSignatureTypeMapper::new(type_inputs(export)),
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
        let mapper = HirSignatureTypeMapper::new(type_inputs(export));
        let result = mapper.map(owner_type, &binders).map_err(|error| {
            CoreProtocolCallableBuildError::InvalidSignatureType { definition, error }
        })?;
        Ok(Self {
            definition,
            signature: SignatureCallableShape::new(Effect::Ordinary, None, parameter_types, result),
        })
    }
}

impl WireEncode for CoreProtocolCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCoreProtocolCallableV1 {
    definition: DecodedCoreProtocolCallableDefinitionV1,
    signature: DecodedSignatureCallableShape,
}

impl DecodedCoreProtocolCallableV1 {
    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<CoreProtocolCallableV1, CoreProtocolCallableValidationError> {
        let definition = resolve_definition(foundation, self.definition)?;
        if foundation
            .definition_origin(definition.origin_subject())
            .is_none()
        {
            return Err(CoreProtocolCallableValidationError::MissingDefinitionOrigin(definition));
        }
        let mut resolver = ProtocolSignatureResolver { foundation };
        let signature = self
            .signature
            .resolve(&mut resolver)
            .map_err(CoreProtocolCallableValidationError::SignatureReference)?;
        validate_definition_signature(foundation, definition, &signature)?;
        Ok(CoreProtocolCallableV1 {
            definition,
            signature,
        })
    }
}

impl WireEncode for DecodedCoreProtocolCallableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.definition.encode(encoder)?;
        encoder.field(2)?;
        self.signature.encode(encoder)
    }
}

impl WireDecode for DecodedCoreProtocolCallableV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            definition: decoder.field(1, DecodedCoreProtocolCallableDefinitionV1::decode)?,
            signature: decoder.field(2, DecodedSignatureCallableShape::decode)?,
        })
    }
}

fn require_core_origin(
    export: &ExportHir,
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<(), CoreProtocolCallableBuildError> {
    if export.cone != ConeIdentity::CORE {
        return Err(CoreProtocolCallableBuildError::NotCore(export.cone));
    }
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

fn type_inputs(export: &ExportHir) -> HirTypeIdentityInputs<'_> {
    HirTypeIdentityInputs {
        types: &export.types,
        function_types: &export.function_types,
        structs: &export.structs,
        struct_applications: &export.struct_applications,
        enums: &export.enums,
        enum_applications: &export.enum_applications,
        classes: &export.classes,
        class_applications: &export.class_applications,
        interfaces: &export.interfaces,
        interface_applications: &export.interface_applications,
        objects: &export.objects,
        intrinsic_core: &export.core_protocols.fundamental_types,
        nominal_identities: &export.nominal_identities,
    }
}

fn resolve_definition(
    foundation: &CanonicalHirFoundation,
    decoded: DecodedCoreProtocolCallableDefinitionV1,
) -> Result<CoreProtocolCallableDefinitionV1, CoreProtocolCallableValidationError> {
    match decoded {
        DecodedCoreProtocolCallableDefinitionV1::Function(id) => foundation
            .function_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::Function(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownFunction(
                *id.as_array(),
            )),
        DecodedCoreProtocolCallableDefinitionV1::GenericFunction(id) => foundation
            .generic_function_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::GenericFunction(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownGenericFunction(
                *id.as_array(),
            )),
        DecodedCoreProtocolCallableDefinitionV1::Constructor(id) => foundation
            .constructor_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::Constructor(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownConstructor(
                *id.as_array(),
            )),
        DecodedCoreProtocolCallableDefinitionV1::GeneratedCallable(id) => foundation
            .generated_callable_by_bytes(id.as_array())
            .map(|(id, _)| CoreProtocolCallableDefinitionV1::GeneratedCallable(id))
            .ok_or(CoreProtocolCallableValidationError::UnknownGeneratedCallable(*id.as_array())),
    }
}

fn validate_definition_signature(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    match definition {
        CoreProtocolCallableDefinitionV1::Function(id) => {
            let (_, source) = foundation
                .function_by_bytes(id.as_array())
                .expect("resolved function remains in the foundation");
            validate_source_function(foundation, definition, source, signature)
        }
        CoreProtocolCallableDefinitionV1::GenericFunction(id) => {
            let (_, source) = foundation
                .generic_function_by_bytes(id.as_array())
                .expect("resolved generic function remains in the foundation");
            validate_source_function(foundation, definition, source, signature)
        }
        CoreProtocolCallableDefinitionV1::Constructor(id) => {
            let (_, source) = foundation
                .constructor_by_bytes(id.as_array())
                .expect("resolved constructor remains in the foundation");
            validate_source_constructor(foundation, definition, source, signature)
        }
        CoreProtocolCallableDefinitionV1::GeneratedCallable(id) => {
            let (_, key) = foundation
                .generated_callable_by_bytes(id.as_array())
                .expect("resolved generated callable remains in the foundation");
            let GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } = key else {
                return Err(
                    CoreProtocolCallableValidationError::UnsupportedGeneratedRole(definition),
                );
            };
            let (_, source) = foundation
                .constructor_by_bytes(constructor.as_array())
                .ok_or(CoreProtocolCallableValidationError::MissingAdapterSource(
                    definition,
                ))?;
            if !signature.parameters().is_empty() {
                return Err(CoreProtocolCallableValidationError::AdapterHasParameters(
                    definition,
                ));
            }
            validate_constructor_common(foundation, definition, source, signature)
        }
    }
}

fn validate_source_function(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    if source.origin() != ConeIdentity::CORE {
        return Err(CoreProtocolCallableValidationError::NonCoreDefinition(
            definition,
        ));
    }
    let DuplicateSignatureKey::Function {
        type_parameter_count,
        receiver,
        parameters,
    } = source.duplicate_signature()
    else {
        return Err(CoreProtocolCallableValidationError::WrongSourceKind(
            definition,
        ));
    };
    let receiver_matches = match (signature.receiver(), receiver) {
        (OptionalSignatureType::Absent, OptionalSignatureType::Absent) => true,
        (OptionalSignatureType::Present(actual), OptionalSignatureType::Present(expected)) => {
            actual == expected
        }
        _ => false,
    };
    if !receiver_matches || signature.parameters() != parameters {
        return Err(CoreProtocolCallableValidationError::SourceSignatureMismatch(definition));
    }
    validate_protocol_binders(
        foundation,
        source,
        signature,
        *type_parameter_count,
        definition,
    )
}

fn validate_source_constructor(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    let DuplicateSignatureKey::Constructor { parameters } = source.duplicate_signature() else {
        return Err(CoreProtocolCallableValidationError::WrongSourceKind(
            definition,
        ));
    };
    if signature.parameters() != parameters {
        return Err(CoreProtocolCallableValidationError::SourceSignatureMismatch(definition));
    }
    validate_constructor_common(foundation, definition, source, signature)
}

fn validate_constructor_common(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
) -> Result<(), CoreProtocolCallableValidationError> {
    if source.origin() != ConeIdentity::CORE {
        return Err(CoreProtocolCallableValidationError::NonCoreDefinition(
            definition,
        ));
    }
    if signature.effect() != Effect::Ordinary
        || !matches!(signature.receiver(), OptionalSignatureType::Absent)
    {
        return Err(CoreProtocolCallableValidationError::InvalidConstructorShape(definition));
    }
    let owner = source
        .owners()
        .owners()
        .last()
        .ok_or(CoreProtocolCallableValidationError::MissingConstructorOwner(definition))?;
    let valid_result = match (owner, signature.result()) {
        (DefinitionOwnerAtom::Type(owner), SignatureTypeKey::Nominal(result)) => owner == result,
        (
            DefinitionOwnerAtom::GenericType(owner),
            SignatureTypeKey::NominalApplication { origin, .. },
        ) => owner == origin,
        _ => false,
    };
    if !valid_result {
        return Err(CoreProtocolCallableValidationError::ConstructorResultMismatch(definition));
    }
    validate_protocol_binders(foundation, source, signature, 0, definition)
}

fn validate_protocol_binders(
    foundation: &CanonicalHirFoundation,
    source: &scoop_identity::SourceDeclarationKey,
    signature: &SignatureCallableShape,
    own_type_parameter_count: u32,
    definition: CoreProtocolCallableDefinitionV1,
) -> Result<(), CoreProtocolCallableValidationError> {
    let mut binder_parameter_counts = Vec::new();
    if own_type_parameter_count > 0 {
        binder_parameter_counts.push(own_type_parameter_count);
    }
    for owner in source.owners().owners().iter().rev() {
        let count = match owner {
            DefinitionOwnerAtom::GenericType(id) => foundation
                .generic_type_by_bytes(id.as_array())
                .map(|(_, source)| source.duplicate_signature().type_parameter_count())
                .ok_or_else(|| {
                    CoreProtocolCallableValidationError::UnknownBinderOwner(owner.clone())
                })?,
            DefinitionOwnerAtom::GenericFunction(id) => foundation
                .generic_function_by_bytes(id.as_array())
                .map(|(_, source)| source.duplicate_signature().type_parameter_count())
                .ok_or_else(|| {
                    CoreProtocolCallableValidationError::UnknownBinderOwner(owner.clone())
                })?,
            DefinitionOwnerAtom::ExtensionProperty(id) => foundation
                .extension_property_by_bytes(id.as_array())
                .map(|(_, source)| source.duplicate_signature().type_parameter_count())
                .ok_or_else(|| {
                    CoreProtocolCallableValidationError::UnknownBinderOwner(owner.clone())
                })?,
            DefinitionOwnerAtom::Type(_)
            | DefinitionOwnerAtom::Function(_)
            | DefinitionOwnerAtom::Constructor(_)
            | DefinitionOwnerAtom::Property(_)
            | DefinitionOwnerAtom::GeneratedCallable(_)
            | DefinitionOwnerAtom::PropertyAccessor(_)
            | DefinitionOwnerAtom::EnumVariant(_) => 0,
        };
        if count > 0 {
            binder_parameter_counts.push(count);
        }
    }
    let mut types = signature.parameters().iter().collect::<Vec<_>>();
    types.push(signature.result());
    if let OptionalSignatureType::Present(receiver) = signature.receiver() {
        types.push(receiver);
    }
    for ty in types {
        validate_protocol_signature_binders(ty, &binder_parameter_counts).map_err(|reason| {
            CoreProtocolCallableValidationError::InvalidBinder { definition, reason }
        })?;
    }
    Ok(())
}

fn validate_protocol_signature_binders(
    signature: &SignatureTypeKey,
    binder_parameter_counts: &[u32],
) -> Result<(), CoreProtocolBinderError> {
    match signature {
        SignatureTypeKey::Nominal(_) => Ok(()),
        SignatureTypeKey::NominalApplication { arguments, .. }
        | SignatureTypeKey::Tuple(arguments) => {
            for argument in arguments.as_slice() {
                validate_protocol_signature_binders(argument, binder_parameter_counts)?;
            }
            Ok(())
        }
        SignatureTypeKey::Function {
            parameters, result, ..
        }
        | SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            for parameter in parameters {
                validate_protocol_signature_binders(parameter, binder_parameter_counts)?;
            }
            validate_protocol_signature_binders(result, binder_parameter_counts)
        }
        SignatureTypeKey::RawPointer(pointee) => {
            validate_protocol_signature_binders(pointee, binder_parameter_counts)
        }
        SignatureTypeKey::Binder { depth, index } => {
            let depth_index = usize::try_from(*depth)
                .map_err(|_| CoreProtocolBinderError::DepthOutOfRange(*depth))?;
            let Some(&count) = binder_parameter_counts.get(depth_index) else {
                return Err(CoreProtocolBinderError::DepthOutOfRange(*depth));
            };
            if *index >= count {
                return Err(CoreProtocolBinderError::IndexOutOfRange {
                    depth: *depth,
                    index: *index,
                    type_parameter_count: count,
                });
            }
            Ok(())
        }
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

struct ProtocolSignatureResolver<'a> {
    foundation: &'a CanonicalHirFoundation,
}

impl PersistentIdResolver<PersistentTypeId> for ProtocolSignatureResolver<'_> {
    type Error = CoreProtocolSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.foundation
            .source_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreProtocolSignatureReferenceError::UnknownSourceType(
                *id.as_array(),
            ))
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for ProtocolSignatureResolver<'_> {
    type Error = CoreProtocolSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        self.foundation
            .generic_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreProtocolSignatureReferenceError::UnknownGenericType(
                *id.as_array(),
            ))
    }
}

#[derive(Debug)]
pub enum CoreProtocolCallableBuildError {
    NotCore(ConeIdentity),
    UnknownFunction(u32),
    UnknownClassConstructor(u32),
    NonSourceFunction(u32),
    MissingDefinitionOrigin(CoreProtocolCallableDefinitionV1),
    TooManyTypeParameters {
        definition: CoreProtocolCallableDefinitionV1,
    },
    TypeParameterCountMismatch {
        definition: CoreProtocolCallableDefinitionV1,
    },
    MissingReceiver {
        definition: CoreProtocolCallableDefinitionV1,
    },
    MissingSourceParameterInterface {
        definition: CoreProtocolCallableDefinitionV1,
    },
    DuplicateSourceParameterInterface {
        definition: CoreProtocolCallableDefinitionV1,
    },
    InvalidSourceParameterInterface {
        definition: CoreProtocolCallableDefinitionV1,
    },
    InvalidSignatureType {
        definition: CoreProtocolCallableDefinitionV1,
        error: crate::HirSignatureTypeMappingError,
    },
    DuplicateSignatureMismatch {
        definition: CoreProtocolCallableDefinitionV1,
    },
    UnknownConstructorOwner {
        definition: CoreProtocolCallableDefinitionV1,
        owner: u32,
    },
    UnknownConstructorSelfApplication {
        definition: CoreProtocolCallableDefinitionV1,
    },
}

impl fmt::Display for CoreProtocolCallableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build core protocol callable: {self:?}")
    }
}

impl std::error::Error for CoreProtocolCallableBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreProtocolSignatureReferenceError {
    UnknownSourceType([u8; 32]),
    UnknownGenericType([u8; 32]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreProtocolBinderError {
    DepthOutOfRange(u32),
    IndexOutOfRange {
        depth: u32,
        index: u32,
        type_parameter_count: u32,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreProtocolCallableValidationError {
    UnknownFunction([u8; 32]),
    UnknownGenericFunction([u8; 32]),
    UnknownConstructor([u8; 32]),
    UnknownGeneratedCallable([u8; 32]),
    MissingDefinitionOrigin(CoreProtocolCallableDefinitionV1),
    SignatureReference(CoreProtocolSignatureReferenceError),
    NonCoreDefinition(CoreProtocolCallableDefinitionV1),
    WrongSourceKind(CoreProtocolCallableDefinitionV1),
    SourceSignatureMismatch(CoreProtocolCallableDefinitionV1),
    InvalidBinder {
        definition: CoreProtocolCallableDefinitionV1,
        reason: CoreProtocolBinderError,
    },
    UnknownBinderOwner(DefinitionOwnerAtom),
    UnsupportedGeneratedRole(CoreProtocolCallableDefinitionV1),
    MissingAdapterSource(CoreProtocolCallableDefinitionV1),
    AdapterHasParameters(CoreProtocolCallableDefinitionV1),
    InvalidConstructorShape(CoreProtocolCallableDefinitionV1),
    MissingConstructorOwner(CoreProtocolCallableDefinitionV1),
    ConstructorResultMismatch(CoreProtocolCallableDefinitionV1),
}

impl fmt::Display for CoreProtocolCallableValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core protocol callable: {self:?}")
    }
}

impl std::error::Error for CoreProtocolCallableValidationError {}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn require_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

#[cfg(test)]
mod tests;
