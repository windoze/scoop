//! Canonical value-namespace targets exported by the trusted core Cone.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    AccessorRole, BindableEntity, ConeIdentity, DecodedOptionalExactOwner,
    DecodedOptionalSignatureType, DecodedPersistentId, DecodedSignatureTypeKey,
    DefinitionOriginSubject, DuplicateSignatureKey, ExactTypeKey, OptionalExactOwner,
    OptionalSignatureType, PersistentExactTypeId, PersistentExportBindingId,
    PersistentExtensionPropertyId, PersistentGenericTypeId, PersistentIdResolver,
    PersistentObjectValueId, PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
    PropertyOwner as PersistentPropertyOwner, SignatureTypeKey, SourceDeclarationKind,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::core_targets::{
    ExactSignatureRelationError, signature_type_for_exact, type_inputs, validate_signature_binders,
};
use crate::{
    CanonicalDirectPublicSurfaceV1, CanonicalHirFoundation, ExportHir, HirPropertyIdentity,
    HirSignatureBinder, HirSignatureTypeMapper, PropertyCapability, PropertyOwner,
    ValidatedHirFoundation,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreValueDefinitionV1 {
    ObjectValue(PersistentObjectValueId),
    Property(PersistentPropertyId),
    ExtensionProperty(PersistentExtensionPropertyId),
}

impl CoreValueDefinitionV1 {
    const fn property_owner(self) -> Option<PersistentPropertyOwner> {
        match self {
            Self::ObjectValue(_) => None,
            Self::Property(id) => Some(PersistentPropertyOwner::Property(id)),
            Self::ExtensionProperty(id) => Some(PersistentPropertyOwner::ExtensionProperty(id)),
        }
    }

    const fn definition_origin(self) -> Option<DefinitionOriginSubject> {
        match self {
            Self::ObjectValue(_) => None,
            Self::Property(id) => Some(DefinitionOriginSubject::Property(id)),
            Self::ExtensionProperty(id) => Some(DefinitionOriginSubject::ExtensionProperty(id)),
        }
    }
}

impl WireEncode for CoreValueDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ObjectValue(id) => encode_value_sum(encoder, 1, id),
            Self::Property(id) => encode_value_sum(encoder, 2, id),
            Self::ExtensionProperty(id) => encode_value_sum(encoder, 3, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCoreValueDefinitionV1 {
    ObjectValue(DecodedPersistentId<PersistentObjectValueId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
    ExtensionProperty(DecodedPersistentId<PersistentExtensionPropertyId>),
}

impl WireEncode for DecodedCoreValueDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ObjectValue(id) => encode_value_sum(encoder, 1, id),
            Self::Property(id) => encode_value_sum(encoder, 2, id),
            Self::ExtensionProperty(id) => encode_value_sum(encoder, 3, id),
        }
    }
}

impl WireDecode for DecodedCoreValueDefinitionV1 {
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
                .map(Self::ObjectValue),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ExtensionProperty),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorePropertyAccessorsV1 {
    ReadOnly {
        getter: PersistentPropertyAccessorId,
    },
    ReadWrite {
        getter: PersistentPropertyAccessorId,
        setter: PersistentPropertyAccessorId,
    },
}

impl CorePropertyAccessorsV1 {
    pub const fn getter(self) -> PersistentPropertyAccessorId {
        match self {
            Self::ReadOnly { getter } | Self::ReadWrite { getter, .. } => getter,
        }
    }

    pub const fn setter(self) -> Option<PersistentPropertyAccessorId> {
        match self {
            Self::ReadOnly { .. } => None,
            Self::ReadWrite { setter, .. } => Some(setter),
        }
    }
}

impl WireEncode for CorePropertyAccessorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ReadOnly { getter } => encode_value_sum(encoder, 1, getter),
            Self::ReadWrite { getter, setter } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                getter.encode(encoder)?;
                encoder.field(2)?;
                setter.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCorePropertyAccessorsV1 {
    ReadOnly {
        getter: DecodedPersistentId<PersistentPropertyAccessorId>,
    },
    ReadWrite {
        getter: DecodedPersistentId<PersistentPropertyAccessorId>,
        setter: DecodedPersistentId<PersistentPropertyAccessorId>,
    },
}

impl WireEncode for DecodedCorePropertyAccessorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ReadOnly { getter } => encode_value_sum(encoder, 1, getter),
            Self::ReadWrite { getter, setter } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                getter.encode(encoder)?;
                encoder.field(2)?;
                setter.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedCorePropertyAccessorsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(|getter| Self::ReadOnly { getter })
            }
            2 => {
                require_sum_length(decoder, fields, 3)?;
                Ok(Self::ReadWrite {
                    getter: decoder.field(1, DecodedPersistentId::decode)?,
                    setter: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorePropertySourceInterfaceV1 {
    receiver: OptionalSignatureType,
    value: SignatureTypeKey,
    accessors: CorePropertyAccessorsV1,
}

impl CorePropertySourceInterfaceV1 {
    pub const fn receiver(&self) -> &OptionalSignatureType {
        &self.receiver
    }

    pub const fn value(&self) -> &SignatureTypeKey {
        &self.value
    }

    pub const fn accessors(&self) -> CorePropertyAccessorsV1 {
        self.accessors
    }
}

impl WireEncode for CorePropertySourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.receiver.encode(encoder)?;
        encoder.field(2)?;
        self.value.encode(encoder)?;
        encoder.field(3)?;
        self.accessors.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCorePropertySourceInterfaceV1 {
    receiver: DecodedOptionalSignatureType,
    value: DecodedSignatureTypeKey,
    accessors: DecodedCorePropertyAccessorsV1,
}

impl WireEncode for DecodedCorePropertySourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.receiver.encode(encoder)?;
        encoder.field(2)?;
        self.value.encode(encoder)?;
        encoder.field(3)?;
        self.accessors.encode(encoder)
    }
}

impl WireDecode for DecodedCorePropertySourceInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            receiver: decoder.field(1, DecodedOptionalSignatureType::decode)?,
            value: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            accessors: decoder.field(3, DecodedCorePropertyAccessorsV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreValueSourceInterfaceV1 {
    ObjectValue { source_type: PersistentTypeId },
    Property(CorePropertySourceInterfaceV1),
}

impl WireEncode for CoreValueSourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ObjectValue { source_type } => encode_value_sum(encoder, 1, source_type),
            Self::Property(property) => encode_value_sum(encoder, 2, property),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedCoreValueSourceInterfaceV1 {
    ObjectValue {
        source_type: DecodedPersistentId<PersistentTypeId>,
    },
    Property(DecodedCorePropertySourceInterfaceV1),
}

impl WireEncode for DecodedCoreValueSourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ObjectValue { source_type } => encode_value_sum(encoder, 1, source_type),
            Self::Property(property) => encode_value_sum(encoder, 2, property),
        }
    }
}

impl WireDecode for DecodedCoreValueSourceInterfaceV1 {
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
                .map(|source_type| Self::ObjectValue { source_type }),
            2 => decoder
                .field(1, DecodedCorePropertySourceInterfaceV1::decode)
                .map(Self::Property),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreExactValueInterfaceV1 {
    ObjectValue(PersistentExactTypeId),
    Property {
        receiver: OptionalExactOwner,
        value: PersistentExactTypeId,
    },
}

impl WireEncode for CoreExactValueInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ObjectValue(exact) => encode_value_sum(encoder, 1, exact),
            Self::Property { receiver, value } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                receiver.encode(encoder)?;
                encoder.field(2)?;
                value.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedCoreExactValueInterfaceV1 {
    ObjectValue(DecodedPersistentId<PersistentExactTypeId>),
    Property {
        receiver: DecodedOptionalExactOwner,
        value: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl WireEncode for DecodedCoreExactValueInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ObjectValue(exact) => encode_value_sum(encoder, 1, exact),
            Self::Property { receiver, value } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                receiver.encode(encoder)?;
                encoder.field(2)?;
                value.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedCoreExactValueInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if fields == 0 {
            return Err(wire_error(
                decoder,
                WireErrorKind::MissingField { field: 0 },
            ));
        }
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                require_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::ObjectValue)
            }
            2 => {
                require_sum_length(decoder, fields, 3)?;
                Ok(Self::Property {
                    receiver: decoder.field(1, DecodedOptionalExactOwner::decode)?,
                    value: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreHirValueCapabilityV1 {
    ParamFreeStrong(CoreExactValueInterfaceV1),
    StructuralUnavailable(CoreExactValueInterfaceV1),
    GenericUnavailable { type_parameter_count: u32 },
}

impl WireEncode for CoreHirValueCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFreeStrong(interface) => encode_value_sum(encoder, 1, interface),
            Self::StructuralUnavailable(interface) => encode_value_sum(encoder, 2, interface),
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
pub enum DecodedCoreHirValueCapabilityV1 {
    ParamFreeStrong(DecodedCoreExactValueInterfaceV1),
    StructuralUnavailable(DecodedCoreExactValueInterfaceV1),
    GenericUnavailable { type_parameter_count: u32 },
}

impl WireEncode for DecodedCoreHirValueCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFreeStrong(interface) => encode_value_sum(encoder, 1, interface),
            Self::StructuralUnavailable(interface) => encode_value_sum(encoder, 2, interface),
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

impl WireDecode for DecodedCoreHirValueCapabilityV1 {
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
                .field(1, DecodedCoreExactValueInterfaceV1::decode)
                .map(Self::ParamFreeStrong),
            2 => decoder
                .field(1, DecodedCoreExactValueInterfaceV1::decode)
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
pub struct CoreValueTargetV1 {
    binding: PersistentExportBindingId,
    definition: CoreValueDefinitionV1,
    source_interface: CoreValueSourceInterfaceV1,
    capability: CoreHirValueCapabilityV1,
}

impl CoreValueTargetV1 {
    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn definition(&self) -> CoreValueDefinitionV1 {
        self.definition
    }

    pub const fn source_interface(&self) -> &CoreValueSourceInterfaceV1 {
        &self.source_interface
    }

    pub const fn capability(&self) -> &CoreHirValueCapabilityV1 {
        &self.capability
    }
}

impl WireEncode for CoreValueTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.source_interface.encode(encoder)?;
        encoder.field(4)?;
        self.capability.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreValueTargetV1 {
    binding: DecodedPersistentId<PersistentExportBindingId>,
    definition: DecodedCoreValueDefinitionV1,
    source_interface: DecodedCoreValueSourceInterfaceV1,
    capability: DecodedCoreHirValueCapabilityV1,
}

impl WireEncode for DecodedCoreValueTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.source_interface.encode(encoder)?;
        encoder.field(4)?;
        self.capability.encode(encoder)
    }
}

impl WireDecode for DecodedCoreValueTargetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            binding: decoder.field(1, DecodedPersistentId::decode)?,
            definition: decoder.field(2, DecodedCoreValueDefinitionV1::decode)?,
            source_interface: decoder.field(3, DecodedCoreValueSourceInterfaceV1::decode)?,
            capability: decoder.field(4, DecodedCoreHirValueCapabilityV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreValueTargetSurfaceV1 {
    targets: Vec<CoreValueTargetV1>,
}

impl CoreValueTargetSurfaceV1 {
    pub(super) fn from_core_export(
        export: &ExportHir,
        protocols: &crate::DefinedCoreProtocols,
    ) -> Result<Self, CoreValueTargetSurfaceBuildError> {
        if export.cone != ConeIdentity::CORE {
            return Err(CoreValueTargetSurfaceBuildError::NotCore(export.cone));
        }
        let values = source_values(export)?;
        let mapper = HirSignatureTypeMapper::new(type_inputs(export, protocols));
        let mut targets = Vec::new();
        for binding in export.export_binding_identities.iter() {
            if binding.key().exporter() != ConeIdentity::CORE {
                return Err(CoreValueTargetSurfaceBuildError::NonCoreBinding(
                    binding.id(),
                ));
            }
            let definition = match binding.key().target() {
                BindableEntity::ObjectValue(id) => CoreValueDefinitionV1::ObjectValue(id),
                BindableEntity::Property(id) => CoreValueDefinitionV1::Property(id),
                BindableEntity::ExtensionProperty(id) => {
                    CoreValueDefinitionV1::ExtensionProperty(id)
                }
                BindableEntity::Type(_)
                | BindableEntity::GenericType(_)
                | BindableEntity::Function(_)
                | BindableEntity::GenericFunction(_)
                | BindableEntity::TypeAlias(_)
                | BindableEntity::EnumVariant(_) => continue,
            };
            let source = *values.get(&definition).ok_or(
                CoreValueTargetSurfaceBuildError::MissingValueDefinition {
                    binding: binding.id(),
                    definition,
                },
            )?;
            let target = match source {
                ValueSource::Object(value) => {
                    build_object_target(export, binding.id(), definition, value)?
                }
                ValueSource::Property(property) => {
                    build_property_target(export, &mapper, binding.id(), definition, property)?
                }
            };
            targets.push(target);
        }
        Self::try_new(targets)
    }

    pub fn try_new(
        mut targets: Vec<CoreValueTargetV1>,
    ) -> Result<Self, CoreValueTargetSurfaceBuildError> {
        targets.sort_by_key(CoreValueTargetV1::binding);
        if let Some(pair) = targets
            .windows(2)
            .find(|pair| pair[0].binding == pair[1].binding)
        {
            return Err(CoreValueTargetSurfaceBuildError::DuplicateBinding(
                pair[0].binding,
            ));
        }
        Ok(Self { targets })
    }

    pub fn targets(&self) -> &[CoreValueTargetV1] {
        &self.targets
    }
}

impl WireEncode for CoreValueTargetSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.targets.len() as u64)?;
        for target in &self.targets {
            target.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedCoreValueTargetSurfaceV1 {
    targets: Vec<DecodedCoreValueTargetV1>,
}

impl DecodedCoreValueTargetSurfaceV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<CoreValueTargetSurfaceV1, CoreValueTargetSurfaceValidationError> {
        self.validate_against(foundation.canonical(), direct_surface)
    }

    pub(super) fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<CoreValueTargetSurfaceV1, CoreValueTargetSurfaceValidationError> {
        let mut expected = Vec::new();
        for binding in direct_surface.bindings() {
            let key = foundation.export_binding_key(*binding).ok_or(
                CoreValueTargetSurfaceValidationError::UnknownDirectSurfaceBinding(*binding),
            )?;
            if matches!(
                key.target(),
                BindableEntity::ObjectValue(_)
                    | BindableEntity::Property(_)
                    | BindableEntity::ExtensionProperty(_)
            ) {
                expected.push(*binding);
            }
        }
        if self.targets.len() != expected.len() {
            return Err(CoreValueTargetSurfaceValidationError::Coverage {
                expected: expected.len(),
                actual: self.targets.len(),
            });
        }
        validate_order(&self.targets)?;
        let mut targets = Vec::with_capacity(self.targets.len());
        for (index, (decoded, expected_binding)) in
            self.targets.into_iter().zip(expected).enumerate()
        {
            let binding = foundation
                .export_binding_id_by_bytes(decoded.binding.as_array())
                .ok_or(CoreValueTargetSurfaceValidationError::UnknownBinding {
                    index,
                    identity: *decoded.binding.as_array(),
                })?;
            if binding != expected_binding {
                return Err(CoreValueTargetSurfaceValidationError::CoverageMismatch {
                    index,
                    expected: expected_binding,
                    actual: binding,
                });
            }
            targets.push(validate_target(
                foundation,
                direct_surface,
                binding,
                decoded,
            )?);
        }
        Ok(CoreValueTargetSurfaceV1 { targets })
    }
}

impl WireEncode for DecodedCoreValueTargetSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.targets.len() as u64)?;
        for target in &self.targets {
            target.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreValueTargetSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCoreValueTargetV1::decode(decoder))
            .map(|targets| Self { targets })
    }
}

#[derive(Clone, Copy)]
enum ValueSource {
    Object(crate::SingletonValueId),
    Property(crate::PropertyId),
}

fn source_values(
    export: &ExportHir,
) -> Result<BTreeMap<CoreValueDefinitionV1, ValueSource>, CoreValueTargetSurfaceBuildError> {
    let mut values = BTreeMap::new();
    for (value, _) in export.singleton_values.iter() {
        let definition =
            CoreValueDefinitionV1::ObjectValue(export.object_value_identities[value].id());
        if values
            .insert(definition, ValueSource::Object(value))
            .is_some()
        {
            return Err(CoreValueTargetSurfaceBuildError::DuplicateValueDefinition(
                definition,
            ));
        }
    }
    for (property, _) in export.properties.iter() {
        let definition = match &export.property_identities[property] {
            HirPropertyIdentity::Ordinary(record) => CoreValueDefinitionV1::Property(record.id()),
            HirPropertyIdentity::Extension(record) => {
                CoreValueDefinitionV1::ExtensionProperty(record.id())
            }
        };
        if values
            .insert(definition, ValueSource::Property(property))
            .is_some()
        {
            return Err(CoreValueTargetSurfaceBuildError::DuplicateValueDefinition(
                definition,
            ));
        }
    }
    Ok(values)
}

fn build_object_target(
    export: &ExportHir,
    binding: PersistentExportBindingId,
    definition: CoreValueDefinitionV1,
    value_id: crate::SingletonValueId,
) -> Result<CoreValueTargetV1, CoreValueTargetSurfaceBuildError> {
    let CoreValueDefinitionV1::ObjectValue(value) = definition else {
        return Err(CoreValueTargetSurfaceBuildError::DefinitionKindMismatch {
            binding,
            definition,
        });
    };
    let declaration = &export.singleton_values[value_id];
    let identity = &export.object_value_identities[value_id];
    if identity.id() != value {
        return Err(CoreValueTargetSurfaceBuildError::ObjectValueIdentityMismatch { binding });
    }
    let source = export.nominal_identities[declaration.declaration]
        .source()
        .ok_or(CoreValueTargetSurfaceBuildError::GeneratedObject { binding })?;
    if source.declaration() != identity.record().key()
        || source.declaration().origin() != ConeIdentity::CORE
        || source.declaration().declaration_kind() != SourceDeclarationKind::Object
    {
        return Err(CoreValueTargetSurfaceBuildError::InvalidObjectSource { binding });
    }
    let source_type = source
        .concrete_id()
        .ok_or(CoreValueTargetSurfaceBuildError::GenericObject { binding })?;
    let object_type = &export.object_types[declaration.object_type];
    let exact = exact_type(export, binding, object_type.canonical_type)?;
    if !exact_key(export, exact).is_some_and(|key| key == &ExactTypeKey::Nominal(source_type)) {
        return Err(CoreValueTargetSurfaceBuildError::ObjectExactMismatch { binding });
    }
    require_build_origin(export, binding, DefinitionOriginSubject::Type(source_type))?;
    let has_type_binding = export.export_binding_identities.iter().any(|record| {
        record.key().exporter() == ConeIdentity::CORE
            && record.key().target() == BindableEntity::Type(source_type)
    });
    if !has_type_binding {
        return Err(CoreValueTargetSurfaceBuildError::MissingObjectTypeBinding {
            binding,
            source_type,
        });
    }
    Ok(CoreValueTargetV1 {
        binding,
        definition,
        source_interface: CoreValueSourceInterfaceV1::ObjectValue { source_type },
        capability: CoreHirValueCapabilityV1::ParamFreeStrong(
            CoreExactValueInterfaceV1::ObjectValue(exact),
        ),
    })
}

fn build_property_target(
    export: &ExportHir,
    mapper: &HirSignatureTypeMapper<'_>,
    binding: PersistentExportBindingId,
    definition: CoreValueDefinitionV1,
    property_id: crate::PropertyId,
) -> Result<CoreValueTargetV1, CoreValueTargetSurfaceBuildError> {
    let property = &export.properties[property_id];
    let identity = &export.property_identities[property_id];
    let source = identity.declaration();
    if source.origin() != ConeIdentity::CORE {
        return Err(CoreValueTargetSurfaceBuildError::NonCoreDefinition {
            binding,
            definition,
        });
    }
    let type_parameters = match (definition, identity, property.owner) {
        (
            CoreValueDefinitionV1::Property(expected),
            HirPropertyIdentity::Ordinary(record),
            PropertyOwner::TopLevel,
        ) if expected == record.id()
            && source.declaration_kind() == SourceDeclarationKind::Property =>
        {
            &[][..]
        }
        (
            CoreValueDefinitionV1::ExtensionProperty(expected),
            HirPropertyIdentity::Extension(record),
            PropertyOwner::Extension(extension),
        ) if expected == record.id()
            && source.declaration_kind() == SourceDeclarationKind::ExtensionProperty =>
        {
            &export.extension_properties[extension].type_params
        }
        _ => {
            return Err(CoreValueTargetSurfaceBuildError::DefinitionKindMismatch {
                binding,
                definition,
            });
        }
    };
    let type_parameter_count = u32::try_from(type_parameters.len())
        .map_err(|_| CoreValueTargetSurfaceBuildError::TooManyTypeParameters { binding })?;
    if source.duplicate_signature().type_parameter_count() != type_parameter_count {
        return Err(CoreValueTargetSurfaceBuildError::TypeParameterCountMismatch { binding });
    }
    let binders = type_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            u32::try_from(index)
                .map(|index| HirSignatureBinder {
                    parameter: parameter.id,
                    depth: 0,
                    index,
                })
                .map_err(|_| CoreValueTargetSurfaceBuildError::TooManyTypeParameters { binding })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let receiver_ty = match property.owner {
        PropertyOwner::TopLevel => None,
        PropertyOwner::Extension(extension) => {
            let declaration = &export.extension_properties[extension];
            if declaration.property != property_id {
                return Err(CoreValueTargetSurfaceBuildError::ExtensionBackReference { binding });
            }
            Some(declaration.receiver_ty)
        }
        PropertyOwner::Class(_)
        | PropertyOwner::Struct(_)
        | PropertyOwner::Enum(_)
        | PropertyOwner::Interface(_)
        | PropertyOwner::Object(_) => {
            return Err(CoreValueTargetSurfaceBuildError::NonPackageProperty { binding });
        }
    };
    let receiver = receiver_ty
        .map(|ty| mapper.map(ty, &binders))
        .transpose()
        .map_err(
            |error| CoreValueTargetSurfaceBuildError::InvalidSignatureType { binding, error },
        )?;
    validate_property_duplicate(source.duplicate_signature(), &receiver)
        .map_err(|()| CoreValueTargetSurfaceBuildError::DuplicateSignatureMismatch { binding })?;
    let value = mapper.map(property.ty, &binders).map_err(|error| {
        CoreValueTargetSurfaceBuildError::InvalidSignatureType { binding, error }
    })?;
    let accessors = build_accessors(export, binding, definition, property.capability)?;
    require_build_origin(
        export,
        binding,
        definition.definition_origin().ok_or(
            CoreValueTargetSurfaceBuildError::DefinitionKindMismatch {
                binding,
                definition,
            },
        )?,
    )?;
    let source_interface = CoreValueSourceInterfaceV1::Property(CorePropertySourceInterfaceV1 {
        receiver: OptionalSignatureType::from_option(receiver),
        value,
        accessors,
    });
    let capability = if type_parameter_count > 0 {
        CoreHirValueCapabilityV1::GenericUnavailable {
            type_parameter_count,
        }
    } else {
        let exact_interface = CoreExactValueInterfaceV1::Property {
            receiver: OptionalExactOwner::from_option(
                receiver_ty
                    .map(|ty| exact_type(export, binding, ty))
                    .transpose()?,
            ),
            value: exact_type(export, binding, property.ty)?,
        };
        if exact_value_roots_are_nominal(export, &exact_interface) {
            CoreHirValueCapabilityV1::ParamFreeStrong(exact_interface)
        } else {
            CoreHirValueCapabilityV1::StructuralUnavailable(exact_interface)
        }
    };
    Ok(CoreValueTargetV1 {
        binding,
        definition,
        source_interface,
        capability,
    })
}

fn build_accessors(
    export: &ExportHir,
    binding: PersistentExportBindingId,
    definition: CoreValueDefinitionV1,
    capability: PropertyCapability,
) -> Result<CorePropertyAccessorsV1, CoreValueTargetSurfaceBuildError> {
    let getter = export.property_accessor_identities[capability.getter()].id();
    require_build_accessor(export, binding, definition, getter, AccessorRole::Getter)?;
    match capability.setter() {
        None => Ok(CorePropertyAccessorsV1::ReadOnly { getter }),
        Some(setter_id) => {
            let setter = export.property_accessor_identities[setter_id].id();
            require_build_accessor(export, binding, definition, setter, AccessorRole::Setter)?;
            Ok(CorePropertyAccessorsV1::ReadWrite { getter, setter })
        }
    }
}

fn require_build_accessor(
    export: &ExportHir,
    binding: PersistentExportBindingId,
    definition: CoreValueDefinitionV1,
    accessor: PersistentPropertyAccessorId,
    role: AccessorRole,
) -> Result<(), CoreValueTargetSurfaceBuildError> {
    let owner = definition.property_owner().ok_or(
        CoreValueTargetSurfaceBuildError::DefinitionKindMismatch {
            binding,
            definition,
        },
    )?;
    let record = export
        .property_getters
        .iter()
        .filter_map(|(id, _)| {
            let identity = &export.property_accessor_identities[id];
            (identity.id() == accessor).then_some(identity.record())
        })
        .chain(export.property_setters.iter().filter_map(|(id, _)| {
            let identity = &export.property_accessor_identities[id];
            (identity.id() == accessor).then_some(identity.record())
        }))
        .next()
        .ok_or(CoreValueTargetSurfaceBuildError::MissingAccessor { binding, accessor })?;
    if record.key().owner() != owner || record.key().role() != role {
        return Err(CoreValueTargetSurfaceBuildError::AccessorRelation { binding, accessor });
    }
    require_build_origin(
        export,
        binding,
        DefinitionOriginSubject::PropertyAccessor(accessor),
    )
}

fn require_build_origin(
    export: &ExportHir,
    binding: PersistentExportBindingId,
    subject: DefinitionOriginSubject,
) -> Result<(), CoreValueTargetSurfaceBuildError> {
    if export.export_definition_origins.get(subject).is_some() {
        Ok(())
    } else {
        Err(CoreValueTargetSurfaceBuildError::MissingDefinitionOrigin { binding, subject })
    }
}

fn exact_type(
    export: &ExportHir,
    binding: PersistentExportBindingId,
    ty: crate::TypeId,
) -> Result<PersistentExactTypeId, CoreValueTargetSurfaceBuildError> {
    export
        .type_identities
        .get(ty)
        .and_then(crate::HirTypeIdentity::exact)
        .map(scoop_identity::CborIdentityRecord::id)
        .ok_or(CoreValueTargetSurfaceBuildError::OpenParamFreeType { binding })
}

fn exact_key(export: &ExportHir, exact: PersistentExactTypeId) -> Option<&ExactTypeKey> {
    export.types.iter().find_map(|(ty, _)| {
        export.type_identities[ty]
            .exact()
            .filter(|record| record.id() == exact)
            .map(scoop_identity::CborIdentityRecord::key)
    })
}

fn exact_value_roots_are_nominal(
    export: &ExportHir,
    interface: &CoreExactValueInterfaceV1,
) -> bool {
    match interface {
        CoreExactValueInterfaceV1::ObjectValue(exact) => {
            matches!(exact_key(export, *exact), Some(ExactTypeKey::Nominal(_)))
        }
        CoreExactValueInterfaceV1::Property { receiver, value } => receiver
            .into_option()
            .into_iter()
            .chain(std::iter::once(*value))
            .all(|exact| matches!(exact_key(export, exact), Some(ExactTypeKey::Nominal(_)))),
    }
}

mod validation;
pub use validation::CoreValueSignatureReferenceError;
use validation::{validate_order, validate_property_duplicate, validate_target};

#[derive(Debug)]
pub enum CoreValueTargetSurfaceBuildError {
    NotCore(ConeIdentity),
    NonCoreBinding(PersistentExportBindingId),
    DuplicateBinding(PersistentExportBindingId),
    DuplicateValueDefinition(CoreValueDefinitionV1),
    MissingValueDefinition {
        binding: PersistentExportBindingId,
        definition: CoreValueDefinitionV1,
    },
    DefinitionKindMismatch {
        binding: PersistentExportBindingId,
        definition: CoreValueDefinitionV1,
    },
    NonCoreDefinition {
        binding: PersistentExportBindingId,
        definition: CoreValueDefinitionV1,
    },
    ObjectValueIdentityMismatch {
        binding: PersistentExportBindingId,
    },
    GeneratedObject {
        binding: PersistentExportBindingId,
    },
    InvalidObjectSource {
        binding: PersistentExportBindingId,
    },
    GenericObject {
        binding: PersistentExportBindingId,
    },
    ObjectExactMismatch {
        binding: PersistentExportBindingId,
    },
    MissingObjectTypeBinding {
        binding: PersistentExportBindingId,
        source_type: PersistentTypeId,
    },
    NonPackageProperty {
        binding: PersistentExportBindingId,
    },
    ExtensionBackReference {
        binding: PersistentExportBindingId,
    },
    TooManyTypeParameters {
        binding: PersistentExportBindingId,
    },
    TypeParameterCountMismatch {
        binding: PersistentExportBindingId,
    },
    DuplicateSignatureMismatch {
        binding: PersistentExportBindingId,
    },
    InvalidSignatureType {
        binding: PersistentExportBindingId,
        error: crate::HirSignatureTypeMappingError,
    },
    OpenParamFreeType {
        binding: PersistentExportBindingId,
    },
    MissingAccessor {
        binding: PersistentExportBindingId,
        accessor: PersistentPropertyAccessorId,
    },
    AccessorRelation {
        binding: PersistentExportBindingId,
        accessor: PersistentPropertyAccessorId,
    },
    MissingDefinitionOrigin {
        binding: PersistentExportBindingId,
        subject: DefinitionOriginSubject,
    },
}

impl fmt::Display for CoreValueTargetSurfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot build core value target surface: {self:?}"
        )
    }
}

impl std::error::Error for CoreValueTargetSurfaceBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreValueTargetSurfaceValidationError {
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
    UnknownObjectValue([u8; 32]),
    UnknownProperty([u8; 32]),
    UnknownExtensionProperty([u8; 32]),
    DefinitionMismatch(PersistentExportBindingId),
    NonCoreDefinition(CoreValueDefinitionV1),
    InvalidObjectSource(PersistentExportBindingId),
    InvalidPropertySource(PersistentExportBindingId),
    InterfaceKindMismatch(PersistentExportBindingId),
    UnknownSourceType([u8; 32]),
    ObjectTypeMismatch(PersistentExportBindingId),
    MissingObjectTypeBinding {
        binding: PersistentExportBindingId,
        source_type: PersistentTypeId,
    },
    MissingDefinitionOrigin(DefinitionOriginSubject),
    ReceiverMismatch(PersistentExportBindingId),
    InvalidBinder {
        binding: PersistentExportBindingId,
        reason: super::CoreCallableBinderError,
    },
    SignatureReference(CoreValueSignatureReferenceError),
    UnknownAccessor([u8; 32]),
    AccessorOwnerMismatch,
    AccessorRoleMismatch(PersistentPropertyAccessorId),
    DuplicateAccessor(PersistentPropertyAccessorId),
    UnknownExactType([u8; 32]),
    ExactRelation {
        binding: PersistentExportBindingId,
        reason: ExactSignatureRelationError,
    },
    ExactInterfaceMismatch(PersistentExportBindingId),
    CapabilityMismatch(PersistentExportBindingId),
}

impl fmt::Display for CoreValueTargetSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core value target surface: {self:?}")
    }
}

impl std::error::Error for CoreValueTargetSurfaceValidationError {}

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

#[cfg(test)]
mod tests;
