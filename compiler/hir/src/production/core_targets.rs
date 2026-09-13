//! Canonical typed callable targets exported by the trusted core Cone.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    BindableEntity, ConeIdentity, DecodedExactCallableSignature, DecodedPersistentId,
    DecodedSignatureCallableShape, DefinitionOriginSubject, DuplicateSignatureKey, Effect,
    ExactCallableSignature, ExactCallableSignatureResolutionError, ExactTypeKey, NonEmptyVec,
    OptionalSignatureType, PersistentExactTypeId, PersistentExportBindingId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentIdResolver, PersistentTypeId,
    SignatureCallableShape, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CanonicalDirectPublicSurfaceV1, CanonicalHirFoundation, ExportHir, HirFunctionIdentity,
    HirSignatureBinder, HirSignatureTypeMapper, HirTypeIdentityInputs, TypeId,
    ValidatedHirFoundation,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreCallableDefinitionV1 {
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
}

impl CoreCallableDefinitionV1 {
    const fn origin_subject(self) -> DefinitionOriginSubject {
        match self {
            Self::Function(id) => DefinitionOriginSubject::Function(id),
            Self::GenericFunction(id) => DefinitionOriginSubject::GenericFunction(id),
        }
    }
}

impl WireEncode for CoreCallableDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCoreCallableDefinitionV1 {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
}

impl WireEncode for DecodedCoreCallableDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_value_sum(encoder, 1, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedCoreCallableDefinitionV1 {
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
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreHirCallableCapabilityV1 {
    ParamFreeStrong(ExactCallableSignature),
    StructuralUnavailable(ExactCallableSignature),
    GenericUnavailable { type_parameter_count: u32 },
}

impl WireEncode for CoreHirCallableCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFreeStrong(signature) => encode_value_sum(encoder, 1, signature),
            Self::StructuralUnavailable(signature) => encode_value_sum(encoder, 2, signature),
            Self::GenericUnavailable {
                type_parameter_count,
            } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedCoreHirCallableCapabilityV1 {
    ParamFreeStrong(DecodedExactCallableSignature),
    StructuralUnavailable(DecodedExactCallableSignature),
    GenericUnavailable { type_parameter_count: u32 },
}

impl WireEncode for DecodedCoreHirCallableCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFreeStrong(signature) => encode_value_sum(encoder, 1, signature),
            Self::StructuralUnavailable(signature) => encode_value_sum(encoder, 2, signature),
            Self::GenericUnavailable {
                type_parameter_count,
            } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.unsigned(u64::from(*type_parameter_count))
            }
        }
    }
}

impl WireDecode for DecodedCoreHirCallableCapabilityV1 {
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
                .field(1, DecodedExactCallableSignature::decode)
                .map(Self::ParamFreeStrong),
            2 => decoder
                .field(1, DecodedExactCallableSignature::decode)
                .map(Self::StructuralUnavailable),
            3 => decoder.field(1, Decoder::u32).map(|type_parameter_count| {
                Self::GenericUnavailable {
                    type_parameter_count,
                }
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreCallableTargetV1 {
    binding: PersistentExportBindingId,
    definition: CoreCallableDefinitionV1,
    signature: SignatureCallableShape,
    capability: CoreHirCallableCapabilityV1,
}

impl CoreCallableTargetV1 {
    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn definition(&self) -> CoreCallableDefinitionV1 {
        self.definition
    }

    pub const fn signature(&self) -> &SignatureCallableShape {
        &self.signature
    }

    pub const fn capability(&self) -> &CoreHirCallableCapabilityV1 {
        &self.capability
    }
}

impl WireEncode for CoreCallableTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.signature.encode(encoder)?;
        encoder.field(4)?;
        self.capability.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreCallableTargetV1 {
    binding: DecodedPersistentId<PersistentExportBindingId>,
    definition: DecodedCoreCallableDefinitionV1,
    signature: DecodedSignatureCallableShape,
    capability: DecodedCoreHirCallableCapabilityV1,
}

impl WireEncode for DecodedCoreCallableTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.signature.encode(encoder)?;
        encoder.field(4)?;
        self.capability.encode(encoder)
    }
}

impl WireDecode for DecodedCoreCallableTargetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            binding: decoder.field(1, DecodedPersistentId::decode)?,
            definition: decoder.field(2, DecodedCoreCallableDefinitionV1::decode)?,
            signature: decoder.field(3, DecodedSignatureCallableShape::decode)?,
            capability: decoder.field(4, DecodedCoreHirCallableCapabilityV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreCallableTargetSurfaceV1 {
    targets: Vec<CoreCallableTargetV1>,
}

impl CoreCallableTargetSurfaceV1 {
    pub fn from_core_export(
        export: &ExportHir,
    ) -> Result<Self, CoreCallableTargetSurfaceBuildError> {
        if export.cone != ConeIdentity::CORE {
            return Err(CoreCallableTargetSurfaceBuildError::NotCore(export.cone));
        }
        let functions = source_functions(export)?;
        let mapper = HirSignatureTypeMapper::new(type_inputs(export));
        let mut targets = Vec::new();
        for binding in export.export_binding_identities.iter() {
            if binding.key().exporter() != ConeIdentity::CORE {
                return Err(CoreCallableTargetSurfaceBuildError::NonCoreBinding(
                    binding.id(),
                ));
            }
            let definition = match binding.key().target() {
                BindableEntity::Function(id) => CoreCallableDefinitionV1::Function(id),
                BindableEntity::GenericFunction(id) => {
                    CoreCallableDefinitionV1::GenericFunction(id)
                }
                BindableEntity::Type(_)
                | BindableEntity::GenericType(_)
                | BindableEntity::ObjectValue(_)
                | BindableEntity::Property(_)
                | BindableEntity::ExtensionProperty(_)
                | BindableEntity::TypeAlias(_)
                | BindableEntity::EnumVariant(_) => continue,
            };
            let function = *functions.get(&definition).ok_or(
                CoreCallableTargetSurfaceBuildError::MissingFunction {
                    binding: binding.id(),
                    definition,
                },
            )?;
            let source = match &export.function_identities[function] {
                HirFunctionIdentity::Source(source) => source.declaration(),
                _ => {
                    return Err(CoreCallableTargetSurfaceBuildError::NonSourceFunction {
                        binding: binding.id(),
                    });
                }
            };
            if source.origin() != ConeIdentity::CORE {
                return Err(CoreCallableTargetSurfaceBuildError::NonCoreDefinition {
                    binding: binding.id(),
                    definition,
                });
            }
            if export
                .export_definition_origins
                .get(definition.origin_subject())
                .is_none()
            {
                return Err(
                    CoreCallableTargetSurfaceBuildError::MissingDefinitionOrigin {
                        binding: binding.id(),
                        definition,
                    },
                );
            }
            targets.push(build_target(
                export,
                &mapper,
                binding.id(),
                definition,
                function,
            )?);
        }
        Self::try_new(targets)
    }

    pub fn try_new(
        mut targets: Vec<CoreCallableTargetV1>,
    ) -> Result<Self, CoreCallableTargetSurfaceBuildError> {
        targets.sort_by_key(CoreCallableTargetV1::binding);
        if let Some(pair) = targets
            .windows(2)
            .find(|pair| pair[0].binding == pair[1].binding)
        {
            return Err(CoreCallableTargetSurfaceBuildError::DuplicateBinding(
                pair[0].binding,
            ));
        }
        Ok(Self { targets })
    }

    pub fn targets(&self) -> &[CoreCallableTargetV1] {
        &self.targets
    }
}

impl WireEncode for CoreCallableTargetSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.targets.len() as u64)?;
        for target in &self.targets {
            target.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedCoreCallableTargetSurfaceV1 {
    targets: Vec<DecodedCoreCallableTargetV1>,
}

impl DecodedCoreCallableTargetSurfaceV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<CoreCallableTargetSurfaceV1, CoreCallableTargetSurfaceValidationError> {
        self.validate_against(foundation.canonical(), direct_surface)
    }

    fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<CoreCallableTargetSurfaceV1, CoreCallableTargetSurfaceValidationError> {
        let mut expected = Vec::new();
        for binding in direct_surface.bindings() {
            let key = foundation.export_binding_key(*binding).ok_or(
                CoreCallableTargetSurfaceValidationError::UnknownDirectSurfaceBinding(*binding),
            )?;
            if matches!(
                key.target(),
                BindableEntity::Function(_) | BindableEntity::GenericFunction(_)
            ) {
                expected.push(*binding);
            }
        }
        if self.targets.len() != expected.len() {
            return Err(CoreCallableTargetSurfaceValidationError::Coverage {
                expected: expected.len(),
                actual: self.targets.len(),
            });
        }
        for (index, pair) in self.targets.windows(2).enumerate() {
            match pair[0].binding.as_array().cmp(pair[1].binding.as_array()) {
                std::cmp::Ordering::Equal => {
                    return Err(CoreCallableTargetSurfaceValidationError::DuplicateBinding {
                        index: index + 1,
                        identity: *pair[1].binding.as_array(),
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(
                        CoreCallableTargetSurfaceValidationError::NonCanonicalOrder {
                            index: index + 1,
                        },
                    );
                }
                std::cmp::Ordering::Less => {}
            }
        }
        let mut targets = Vec::with_capacity(self.targets.len());
        for (index, (decoded, expected_binding)) in
            self.targets.into_iter().zip(expected).enumerate()
        {
            let binding = foundation
                .export_binding_id_by_bytes(decoded.binding.as_array())
                .ok_or(CoreCallableTargetSurfaceValidationError::UnknownBinding {
                    index,
                    identity: *decoded.binding.as_array(),
                })?;
            if binding != expected_binding {
                return Err(CoreCallableTargetSurfaceValidationError::CoverageMismatch {
                    index,
                    expected: expected_binding,
                    actual: binding,
                });
            }
            targets.push(validate_target(foundation, binding, decoded)?);
        }
        Ok(CoreCallableTargetSurfaceV1 { targets })
    }
}

impl WireEncode for DecodedCoreCallableTargetSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.targets.len() as u64)?;
        for target in &self.targets {
            target.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreCallableTargetSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCoreCallableTargetV1::decode(decoder))
            .map(|targets| Self { targets })
    }
}

fn source_functions(
    export: &ExportHir,
) -> Result<
    BTreeMap<CoreCallableDefinitionV1, crate::FunctionId>,
    CoreCallableTargetSurfaceBuildError,
> {
    let mut functions = BTreeMap::new();
    for (function, identity) in export.function_identities.iter() {
        let HirFunctionIdentity::Source(source) = identity else {
            continue;
        };
        let definition = match source {
            crate::HirSourceFunctionIdentity::Plain(record) => {
                CoreCallableDefinitionV1::Function(record.id())
            }
            crate::HirSourceFunctionIdentity::Generic(record) => {
                CoreCallableDefinitionV1::GenericFunction(record.id())
            }
        };
        if functions.insert(definition, function).is_some() {
            return Err(CoreCallableTargetSurfaceBuildError::DuplicateFunction(
                definition,
            ));
        }
    }
    Ok(functions)
}

fn build_target(
    export: &ExportHir,
    mapper: &HirSignatureTypeMapper<'_>,
    binding: PersistentExportBindingId,
    definition: CoreCallableDefinitionV1,
    function: crate::FunctionId,
) -> Result<CoreCallableTargetV1, CoreCallableTargetSurfaceBuildError> {
    let declaration = &export.functions[function];
    let parameters = declaration.type_params();
    let type_parameter_count = u32::try_from(parameters.len())
        .map_err(|_| CoreCallableTargetSurfaceBuildError::TooManyTypeParameters { binding })?;
    let binders = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            u32::try_from(index)
                .map(|index| HirSignatureBinder {
                    parameter: parameter.id,
                    depth: 0,
                    index,
                })
                .map_err(|_| CoreCallableTargetSurfaceBuildError::TooManyTypeParameters { binding })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source = match &export.function_identities[function] {
        HirFunctionIdentity::Source(source) => source.declaration(),
        _ => {
            return Err(CoreCallableTargetSurfaceBuildError::NonSourceFunction { binding });
        }
    };
    if source.duplicate_signature().type_parameter_count() != type_parameter_count {
        return Err(CoreCallableTargetSurfaceBuildError::TypeParameterCountMismatch { binding });
    }
    let (receiver, parameters) = split_receiver(
        binding,
        source.duplicate_signature().receiver_is_present(),
        &declaration.params,
    )?;
    let receiver_shape = receiver
        .map(|parameter| mapper.map(parameter.ty, &binders))
        .transpose()
        .map_err(
            |error| CoreCallableTargetSurfaceBuildError::InvalidSignatureType { binding, error },
        )?;
    let parameter_shapes = parameters
        .iter()
        .map(|parameter| mapper.map(parameter.ty, &binders))
        .collect::<Result<Vec<_>, _>>()
        .map_err(
            |error| CoreCallableTargetSurfaceBuildError::InvalidSignatureType { binding, error },
        )?;
    validate_duplicate_shape(
        source.duplicate_signature(),
        &receiver_shape,
        &parameter_shapes,
    )
    .map_err(|_| CoreCallableTargetSurfaceBuildError::DuplicateSignatureMismatch { binding })?;
    let result = mapper
        .map(declaration.return_ty, &binders)
        .map_err(
            |error| CoreCallableTargetSurfaceBuildError::InvalidSignatureType { binding, error },
        )?;
    let effect = function_effect(declaration.is_suspend);
    let signature = SignatureCallableShape::new(effect, receiver_shape, parameter_shapes, result);
    let capability = if type_parameter_count > 0 {
        CoreHirCallableCapabilityV1::GenericUnavailable {
            type_parameter_count,
        }
    } else {
        let exact = exact_signature(export, binding, declaration, receiver, parameters, effect)?;
        if exact_signature_is_nominal(export, &exact) {
            CoreHirCallableCapabilityV1::ParamFreeStrong(exact)
        } else {
            CoreHirCallableCapabilityV1::StructuralUnavailable(exact)
        }
    };
    Ok(CoreCallableTargetV1 {
        binding,
        definition,
        signature,
        capability,
    })
}

pub(super) fn type_inputs(export: &ExportHir) -> HirTypeIdentityInputs<'_> {
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
        intrinsic_core: &export.intrinsic_type_core,
        nominal_identities: &export.nominal_identities,
    }
}

fn split_receiver(
    binding: PersistentExportBindingId,
    has_receiver: bool,
    parameters: &[crate::Param],
) -> Result<(Option<&crate::Param>, &[crate::Param]), CoreCallableTargetSurfaceBuildError> {
    if has_receiver {
        parameters
            .split_first()
            .map(|(receiver, parameters)| (Some(receiver), parameters))
            .ok_or(CoreCallableTargetSurfaceBuildError::MissingReceiver { binding })
    } else {
        Ok((None, parameters))
    }
}

fn exact_signature(
    export: &ExportHir,
    binding: PersistentExportBindingId,
    declaration: &crate::Function,
    receiver: Option<&crate::Param>,
    parameters: &[crate::Param],
    effect: Effect,
) -> Result<ExactCallableSignature, CoreCallableTargetSurfaceBuildError> {
    Ok(ExactCallableSignature::new(
        effect,
        receiver
            .map(|parameter| exact_type(export, binding, parameter.ty))
            .transpose()?,
        parameters
            .iter()
            .map(|parameter| exact_type(export, binding, parameter.ty))
            .collect::<Result<Vec<_>, _>>()?,
        exact_type(export, binding, declaration.return_ty)?,
    ))
}

fn exact_type(
    export: &ExportHir,
    binding: PersistentExportBindingId,
    ty: TypeId,
) -> Result<PersistentExactTypeId, CoreCallableTargetSurfaceBuildError> {
    export
        .type_identities
        .get(ty)
        .and_then(crate::HirTypeIdentity::exact)
        .map(scoop_identity::CborIdentityRecord::id)
        .ok_or(CoreCallableTargetSurfaceBuildError::OpenParamFreeType { binding })
}

fn exact_signature_is_nominal(export: &ExportHir, signature: &ExactCallableSignature) -> bool {
    signature
        .receiver()
        .into_option()
        .into_iter()
        .chain(signature.parameters().iter().copied())
        .chain(std::iter::once(signature.result()))
        .all(|exact| {
            export.types.iter().any(|(ty, _)| {
                export.type_identities[ty].exact().is_some_and(|record| {
                    record.id() == exact && matches!(record.key(), ExactTypeKey::Nominal(_))
                })
            })
        })
}

fn validate_target(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    decoded: DecodedCoreCallableTargetV1,
) -> Result<CoreCallableTargetV1, CoreCallableTargetSurfaceValidationError> {
    let binding_key = foundation.export_binding_key(binding).ok_or(
        CoreCallableTargetSurfaceValidationError::UnknownBinding {
            index: 0,
            identity: *binding.as_array(),
        },
    )?;
    if binding_key.exporter() != ConeIdentity::CORE {
        return Err(CoreCallableTargetSurfaceValidationError::NonCoreBinding(
            binding,
        ));
    }
    let (definition, source) = resolve_definition(
        foundation,
        binding,
        binding_key.target(),
        decoded.definition,
    )?;
    if !super::direct_binding_matches_source(binding_key, source) {
        return Err(CoreCallableTargetSurfaceValidationError::BindingSourceMismatch(binding));
    }
    if source.origin() != ConeIdentity::CORE {
        return Err(CoreCallableTargetSurfaceValidationError::NonCoreDefinition(
            definition,
        ));
    }
    if foundation
        .definition_origin(definition.origin_subject())
        .is_none()
    {
        return Err(CoreCallableTargetSurfaceValidationError::MissingDefinitionOrigin(definition));
    }
    let mut resolver = FoundationTypeResolver { foundation };
    let signature = decoded
        .signature
        .resolve(&mut resolver)
        .map_err(CoreCallableTargetSurfaceValidationError::SignatureReference)?;
    let (type_parameter_count, receiver, parameters) = match source.duplicate_signature() {
        DuplicateSignatureKey::Function {
            type_parameter_count,
            receiver,
            parameters,
        } => (*type_parameter_count, receiver, parameters),
        DuplicateSignatureKey::Nominal { .. }
        | DuplicateSignatureKey::Constructor { .. }
        | DuplicateSignatureKey::Property { .. }
        | DuplicateSignatureKey::TypeAlias => {
            return Err(
                CoreCallableTargetSurfaceValidationError::NonFunctionDefinition(definition),
            );
        }
    };
    if signature.receiver() != receiver || signature.parameters() != parameters {
        return Err(CoreCallableTargetSurfaceValidationError::SignatureMismatch(
            binding,
        ));
    }
    validate_binders(&signature, type_parameter_count).map_err(|reason| {
        CoreCallableTargetSurfaceValidationError::InvalidBinder { binding, reason }
    })?;
    let capability = validate_capability(
        foundation,
        binding,
        type_parameter_count,
        &signature,
        decoded.capability,
    )?;
    Ok(CoreCallableTargetV1 {
        binding,
        definition,
        signature,
        capability,
    })
}

fn resolve_definition(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    target: BindableEntity,
    decoded: DecodedCoreCallableDefinitionV1,
) -> Result<
    (
        CoreCallableDefinitionV1,
        &scoop_identity::SourceDeclarationKey,
    ),
    CoreCallableTargetSurfaceValidationError,
> {
    match decoded {
        DecodedCoreCallableDefinitionV1::Function(decoded) => {
            let (id, source) = foundation.function_by_bytes(decoded.as_array()).ok_or(
                CoreCallableTargetSurfaceValidationError::UnknownFunction(*decoded.as_array()),
            )?;
            if target != BindableEntity::Function(id) {
                return Err(CoreCallableTargetSurfaceValidationError::DefinitionMismatch(binding));
            }
            Ok((CoreCallableDefinitionV1::Function(id), source))
        }
        DecodedCoreCallableDefinitionV1::GenericFunction(decoded) => {
            let (id, source) = foundation
                .generic_function_by_bytes(decoded.as_array())
                .ok_or(
                    CoreCallableTargetSurfaceValidationError::UnknownGenericFunction(
                        *decoded.as_array(),
                    ),
                )?;
            if target != BindableEntity::GenericFunction(id) {
                return Err(CoreCallableTargetSurfaceValidationError::DefinitionMismatch(binding));
            }
            Ok((CoreCallableDefinitionV1::GenericFunction(id), source))
        }
    }
}

fn validate_capability(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    type_parameter_count: u32,
    signature: &SignatureCallableShape,
    decoded: DecodedCoreHirCallableCapabilityV1,
) -> Result<CoreHirCallableCapabilityV1, CoreCallableTargetSurfaceValidationError> {
    match decoded {
        DecodedCoreHirCallableCapabilityV1::GenericUnavailable {
            type_parameter_count: actual,
        } => {
            if actual == 0 || actual != type_parameter_count {
                return Err(CoreCallableTargetSurfaceValidationError::CapabilityMismatch(binding));
            }
            Ok(CoreHirCallableCapabilityV1::GenericUnavailable {
                type_parameter_count: actual,
            })
        }
        DecodedCoreHirCallableCapabilityV1::ParamFreeStrong(decoded) => {
            if type_parameter_count != 0 {
                return Err(CoreCallableTargetSurfaceValidationError::CapabilityMismatch(binding));
            }
            let exact = validate_exact_signature(foundation, binding, signature, decoded)?;
            if !exact_roots_are_nominal(foundation, &exact) {
                return Err(CoreCallableTargetSurfaceValidationError::CapabilityMismatch(binding));
            }
            Ok(CoreHirCallableCapabilityV1::ParamFreeStrong(exact))
        }
        DecodedCoreHirCallableCapabilityV1::StructuralUnavailable(decoded) => {
            if type_parameter_count != 0 {
                return Err(CoreCallableTargetSurfaceValidationError::CapabilityMismatch(binding));
            }
            let exact = validate_exact_signature(foundation, binding, signature, decoded)?;
            if exact_roots_are_nominal(foundation, &exact) {
                return Err(CoreCallableTargetSurfaceValidationError::CapabilityMismatch(binding));
            }
            Ok(CoreHirCallableCapabilityV1::StructuralUnavailable(exact))
        }
    }
}

fn validate_exact_signature(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    signature: &SignatureCallableShape,
    decoded: DecodedExactCallableSignature,
) -> Result<ExactCallableSignature, CoreCallableTargetSurfaceValidationError> {
    let mut resolver = FoundationTypeResolver { foundation };
    let exact = decoded
        .resolve(&mut resolver)
        .map_err(|error| match error {
            ExactCallableSignatureResolutionError::Reference(error) => {
                CoreCallableTargetSurfaceValidationError::SignatureReference(error)
            }
            ExactCallableSignatureResolutionError::Allocation => {
                CoreCallableTargetSurfaceValidationError::ExactSignatureAllocation
            }
        })?;
    let replayed = signature_for_exact_callable(foundation, &exact).map_err(|reason| {
        CoreCallableTargetSurfaceValidationError::InvalidExactSignature { binding, reason }
    })?;
    if &replayed != signature {
        return Err(CoreCallableTargetSurfaceValidationError::ExactSignatureMismatch(binding));
    }
    Ok(exact)
}

fn signature_for_exact_callable(
    foundation: &CanonicalHirFoundation,
    signature: &ExactCallableSignature,
) -> Result<SignatureCallableShape, ExactSignatureRelationError> {
    let mut active = BTreeSet::new();
    Ok(SignatureCallableShape::new(
        signature.effect(),
        signature
            .receiver()
            .into_option()
            .map(|exact| signature_for_exact(foundation, exact, &mut active))
            .transpose()?,
        signature
            .parameters()
            .iter()
            .map(|exact| signature_for_exact(foundation, *exact, &mut active))
            .collect::<Result<Vec<_>, _>>()?,
        signature_for_exact(foundation, signature.result(), &mut active)?,
    ))
}

pub(super) fn signature_type_for_exact(
    foundation: &CanonicalHirFoundation,
    exact: PersistentExactTypeId,
) -> Result<SignatureTypeKey, ExactSignatureRelationError> {
    signature_for_exact(foundation, exact, &mut BTreeSet::new())
}

fn signature_for_exact(
    foundation: &CanonicalHirFoundation,
    exact: PersistentExactTypeId,
    active: &mut BTreeSet<PersistentExactTypeId>,
) -> Result<SignatureTypeKey, ExactSignatureRelationError> {
    if !active.insert(exact) {
        return Err(ExactSignatureRelationError::Cycle(exact));
    }
    let key = foundation
        .exact_type_key(exact)
        .ok_or(ExactSignatureRelationError::UnknownExact(exact))?;
    let signature = match key {
        ExactTypeKey::Nominal(id) => SignatureTypeKey::Nominal(*id),
        ExactTypeKey::NominalApplication { origin, arguments } => {
            SignatureTypeKey::NominalApplication {
                origin: *origin,
                arguments: NonEmptyVec::new(
                    arguments
                        .as_slice()
                        .iter()
                        .map(|argument| signature_for_exact(foundation, *argument, active))
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .map_err(|_| ExactSignatureRelationError::EmptyExactAggregate(exact))?,
            }
        }
        ExactTypeKey::Tuple(elements) => SignatureTypeKey::Tuple(
            NonEmptyVec::new(
                elements
                    .as_slice()
                    .iter()
                    .map(|element| signature_for_exact(foundation, *element, active))
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .map_err(|_| ExactSignatureRelationError::EmptyExactAggregate(exact))?,
        ),
        ExactTypeKey::Function {
            effect,
            parameters,
            result,
        } => SignatureTypeKey::Function {
            effect: *effect,
            parameters: parameters
                .iter()
                .map(|parameter| signature_for_exact(foundation, *parameter, active))
                .collect::<Result<Vec<_>, _>>()?,
            result: Box::new(signature_for_exact(foundation, *result, active)?),
        },
        ExactTypeKey::RawPointer(pointee) => SignatureTypeKey::RawPointer(Box::new(
            signature_for_exact(foundation, *pointee, active)?,
        )),
        ExactTypeKey::NativeFunctionPointer {
            calling_convention,
            parameters,
            result,
        } => SignatureTypeKey::NativeFunctionPointer {
            calling_convention: *calling_convention,
            parameters: parameters
                .iter()
                .map(|parameter| signature_for_exact(foundation, *parameter, active))
                .collect::<Result<Vec<_>, _>>()?,
            result: Box::new(signature_for_exact(foundation, *result, active)?),
        },
    };
    active.remove(&exact);
    Ok(signature)
}

fn exact_roots_are_nominal(
    foundation: &CanonicalHirFoundation,
    signature: &ExactCallableSignature,
) -> bool {
    signature
        .receiver()
        .into_option()
        .into_iter()
        .chain(signature.parameters().iter().copied())
        .chain(std::iter::once(signature.result()))
        .all(|exact| {
            matches!(
                foundation.exact_type_key(exact),
                Some(ExactTypeKey::Nominal(_))
            )
        })
}

fn validate_duplicate_shape(
    duplicate: &DuplicateSignatureKey,
    receiver: &Option<SignatureTypeKey>,
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

fn validate_binders(
    signature: &SignatureCallableShape,
    type_parameter_count: u32,
) -> Result<(), CoreCallableBinderError> {
    let mut values = signature.parameters().iter().collect::<Vec<_>>();
    values.push(signature.result());
    if let OptionalSignatureType::Present(receiver) = signature.receiver() {
        values.push(receiver.as_ref());
    }
    for value in values {
        validate_signature_binders(value, type_parameter_count)?;
    }
    Ok(())
}

pub(super) fn validate_signature_binders(
    signature: &SignatureTypeKey,
    type_parameter_count: u32,
) -> Result<(), CoreCallableBinderError> {
    match signature {
        SignatureTypeKey::Nominal(_) => Ok(()),
        SignatureTypeKey::NominalApplication { arguments, .. }
        | SignatureTypeKey::Tuple(arguments) => {
            for argument in arguments.as_slice() {
                validate_signature_binders(argument, type_parameter_count)?;
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
                validate_signature_binders(parameter, type_parameter_count)?;
            }
            validate_signature_binders(result, type_parameter_count)
        }
        SignatureTypeKey::RawPointer(pointee) => {
            validate_signature_binders(pointee, type_parameter_count)
        }
        SignatureTypeKey::Binder { depth, index } if *depth != 0 => {
            Err(CoreCallableBinderError::NonLocalDepth(*depth))
        }
        SignatureTypeKey::Binder { index, .. } if *index >= type_parameter_count => {
            Err(CoreCallableBinderError::IndexOutOfRange {
                index: *index,
                type_parameter_count,
            })
        }
        SignatureTypeKey::Binder { .. } => Ok(()),
    }
}

fn function_effect(is_suspend: bool) -> Effect {
    if is_suspend {
        Effect::Suspend
    } else {
        Effect::Ordinary
    }
}

struct FoundationTypeResolver<'a> {
    foundation: &'a CanonicalHirFoundation,
}

impl PersistentIdResolver<PersistentTypeId> for FoundationTypeResolver<'_> {
    type Error = CoreCallableSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.foundation
            .source_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreCallableSignatureReferenceError::UnknownSourceType(
                *id.as_array(),
            ))
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for FoundationTypeResolver<'_> {
    type Error = CoreCallableSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        self.foundation
            .generic_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreCallableSignatureReferenceError::UnknownGenericType(
                *id.as_array(),
            ))
    }
}

impl PersistentIdResolver<PersistentExactTypeId> for FoundationTypeResolver<'_> {
    type Error = CoreCallableSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.foundation
            .exact_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreCallableSignatureReferenceError::UnknownExactType(
                *id.as_array(),
            ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreCallableBinderError {
    NonLocalDepth(u32),
    IndexOutOfRange {
        index: u32,
        type_parameter_count: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreCallableSignatureReferenceError {
    UnknownSourceType([u8; 32]),
    UnknownGenericType([u8; 32]),
    UnknownExactType([u8; 32]),
}

impl fmt::Display for CoreCallableSignatureReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, identity) = match self {
            Self::UnknownSourceType(id) => ("source type", id),
            Self::UnknownGenericType(id) => ("generic type", id),
            Self::UnknownExactType(id) => ("exact type", id),
        };
        write!(
            formatter,
            "core callable signature references unknown {kind} {}",
            Hex(identity)
        )
    }
}

impl std::error::Error for CoreCallableSignatureReferenceError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactSignatureRelationError {
    UnknownExact(PersistentExactTypeId),
    Cycle(PersistentExactTypeId),
    EmptyExactAggregate(PersistentExactTypeId),
}

#[derive(Debug)]
pub enum CoreCallableTargetSurfaceBuildError {
    NotCore(ConeIdentity),
    NonCoreBinding(PersistentExportBindingId),
    NonCoreDefinition {
        binding: PersistentExportBindingId,
        definition: CoreCallableDefinitionV1,
    },
    DuplicateBinding(PersistentExportBindingId),
    DuplicateFunction(CoreCallableDefinitionV1),
    MissingFunction {
        binding: PersistentExportBindingId,
        definition: CoreCallableDefinitionV1,
    },
    MissingDefinitionOrigin {
        binding: PersistentExportBindingId,
        definition: CoreCallableDefinitionV1,
    },
    NonSourceFunction {
        binding: PersistentExportBindingId,
    },
    TooManyTypeParameters {
        binding: PersistentExportBindingId,
    },
    TypeParameterCountMismatch {
        binding: PersistentExportBindingId,
    },
    MissingReceiver {
        binding: PersistentExportBindingId,
    },
    InvalidSignatureType {
        binding: PersistentExportBindingId,
        error: crate::HirSignatureTypeMappingError,
    },
    DuplicateSignatureMismatch {
        binding: PersistentExportBindingId,
    },
    OpenParamFreeType {
        binding: PersistentExportBindingId,
    },
}

impl fmt::Display for CoreCallableTargetSurfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCore(cone) => write!(
                formatter,
                "core callable target surface requires the reserved core Cone, found {cone}"
            ),
            Self::NonCoreBinding(binding) => {
                write!(
                    formatter,
                    "callable binding {binding} is not exported by core"
                )
            }
            Self::NonCoreDefinition {
                binding,
                definition,
            } => write!(
                formatter,
                "core binding {binding} targets non-core definition {definition:?}"
            ),
            Self::DuplicateBinding(binding) => {
                write!(formatter, "duplicate core callable binding {binding}")
            }
            Self::DuplicateFunction(definition) => {
                write!(
                    formatter,
                    "duplicate core callable definition {definition:?}"
                )
            }
            Self::MissingFunction {
                binding,
                definition,
            } => write!(
                formatter,
                "core binding {binding} has no HIR function for {definition:?}"
            ),
            Self::MissingDefinitionOrigin {
                binding,
                definition,
            } => write!(
                formatter,
                "core binding {binding} has no definition origin for {definition:?}"
            ),
            Self::NonSourceFunction { binding } => {
                write!(
                    formatter,
                    "core binding {binding} targets a non-source function"
                )
            }
            Self::TooManyTypeParameters { binding } => write!(
                formatter,
                "core binding {binding} exceeds the callable type-parameter schema"
            ),
            Self::TypeParameterCountMismatch { binding } => write!(
                formatter,
                "core binding {binding} disagrees with its source type-parameter count"
            ),
            Self::MissingReceiver { binding } => {
                write!(
                    formatter,
                    "core extension binding {binding} has no receiver parameter"
                )
            }
            Self::InvalidSignatureType { binding, error } => {
                write!(
                    formatter,
                    "core binding {binding} has an invalid signature: {error}"
                )
            }
            Self::DuplicateSignatureMismatch { binding } => write!(
                formatter,
                "core binding {binding} disagrees with its source duplicate signature"
            ),
            Self::OpenParamFreeType { binding } => write!(
                formatter,
                "parameter-free core binding {binding} contains an open signature type"
            ),
        }
    }
}

impl std::error::Error for CoreCallableTargetSurfaceBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreCallableTargetSurfaceValidationError {
    Coverage {
        expected: usize,
        actual: usize,
    },
    UnknownDirectSurfaceBinding(PersistentExportBindingId),
    DuplicateBinding {
        index: usize,
        identity: [u8; 32],
    },
    NonCanonicalOrder {
        index: usize,
    },
    UnknownBinding {
        index: usize,
        identity: [u8; 32],
    },
    CoverageMismatch {
        index: usize,
        expected: PersistentExportBindingId,
        actual: PersistentExportBindingId,
    },
    NonCoreBinding(PersistentExportBindingId),
    BindingSourceMismatch(PersistentExportBindingId),
    UnknownFunction([u8; 32]),
    UnknownGenericFunction([u8; 32]),
    DefinitionMismatch(PersistentExportBindingId),
    NonCoreDefinition(CoreCallableDefinitionV1),
    MissingDefinitionOrigin(CoreCallableDefinitionV1),
    NonFunctionDefinition(CoreCallableDefinitionV1),
    SignatureReference(CoreCallableSignatureReferenceError),
    SignatureMismatch(PersistentExportBindingId),
    InvalidBinder {
        binding: PersistentExportBindingId,
        reason: CoreCallableBinderError,
    },
    CapabilityMismatch(PersistentExportBindingId),
    ExactSignatureAllocation,
    InvalidExactSignature {
        binding: PersistentExportBindingId,
        reason: ExactSignatureRelationError,
    },
    ExactSignatureMismatch(PersistentExportBindingId),
}

impl fmt::Display for CoreCallableTargetSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core callable target surface: {self:?}")
    }
}

impl std::error::Error for CoreCallableTargetSurfaceValidationError {}

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

struct Hex<'a>(&'a [u8; 32]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
