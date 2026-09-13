//! Canonical type-namespace targets exported by the trusted core Cone.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    BindableEntity, ConeIdentity, DecodedPersistentId, DefinitionOriginSubject, ExactTypeKey,
    PersistentExactTypeId, PersistentExportBindingId, PersistentGenericTypeId,
    PersistentTypeAliasId, PersistentTypeId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CanonicalDirectPublicSurfaceV1, CanonicalHirFoundation, ExportHir, HirNominalIdentity,
    HirSourceNominalIdentity, ValidatedHirFoundation,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreTypeDefinitionV1 {
    Type(PersistentTypeId),
    GenericType(PersistentGenericTypeId),
    TypeAlias(PersistentTypeAliasId),
}

impl CoreTypeDefinitionV1 {
    const fn origin_subject(self) -> DefinitionOriginSubject {
        match self {
            Self::Type(id) => DefinitionOriginSubject::Type(id),
            Self::GenericType(id) => DefinitionOriginSubject::GenericType(id),
            Self::TypeAlias(id) => DefinitionOriginSubject::TypeAlias(id),
        }
    }
}

impl WireEncode for CoreTypeDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Type(id) => encode_value_sum(encoder, 1, id),
            Self::GenericType(id) => encode_value_sum(encoder, 2, id),
            Self::TypeAlias(id) => encode_value_sum(encoder, 3, id),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCoreTypeDefinitionV1 {
    Type(DecodedPersistentId<PersistentTypeId>),
    GenericType(DecodedPersistentId<PersistentGenericTypeId>),
    TypeAlias(DecodedPersistentId<PersistentTypeAliasId>),
}

impl WireEncode for DecodedCoreTypeDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Type(id) => encode_value_sum(encoder, 1, id),
            Self::GenericType(id) => encode_value_sum(encoder, 2, id),
            Self::TypeAlias(id) => encode_value_sum(encoder, 3, id),
        }
    }
}

impl WireDecode for DecodedCoreTypeDefinitionV1 {
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
                .map(Self::Type),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericType),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::TypeAlias),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreHirTypeCapabilityV1 {
    ParamFreeStrong(PersistentExactTypeId),
    StructuralUnavailable(PersistentExactTypeId),
    GenericUnavailable { type_parameter_count: u32 },
}

impl WireEncode for CoreHirTypeCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFreeStrong(exact) => encode_value_sum(encoder, 1, exact),
            Self::StructuralUnavailable(exact) => encode_value_sum(encoder, 2, exact),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedCoreHirTypeCapabilityV1 {
    ParamFreeStrong(DecodedPersistentId<PersistentExactTypeId>),
    StructuralUnavailable(DecodedPersistentId<PersistentExactTypeId>),
    GenericUnavailable { type_parameter_count: u32 },
}

impl WireEncode for DecodedCoreHirTypeCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFreeStrong(exact) => encode_value_sum(encoder, 1, exact),
            Self::StructuralUnavailable(exact) => encode_value_sum(encoder, 2, exact),
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

impl WireDecode for DecodedCoreHirTypeCapabilityV1 {
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
                .map(Self::ParamFreeStrong),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
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
pub struct CoreTypeTargetV1 {
    binding: PersistentExportBindingId,
    definition: CoreTypeDefinitionV1,
    capability: CoreHirTypeCapabilityV1,
}

impl CoreTypeTargetV1 {
    pub const fn binding(&self) -> PersistentExportBindingId {
        self.binding
    }

    pub const fn definition(&self) -> CoreTypeDefinitionV1 {
        self.definition
    }

    pub const fn capability(&self) -> CoreHirTypeCapabilityV1 {
        self.capability
    }
}

impl WireEncode for CoreTypeTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.capability.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedCoreTypeTargetV1 {
    binding: DecodedPersistentId<PersistentExportBindingId>,
    definition: DecodedCoreTypeDefinitionV1,
    capability: DecodedCoreHirTypeCapabilityV1,
}

impl WireEncode for DecodedCoreTypeTargetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.binding.encode(encoder)?;
        encoder.field(2)?;
        self.definition.encode(encoder)?;
        encoder.field(3)?;
        self.capability.encode(encoder)
    }
}

impl WireDecode for DecodedCoreTypeTargetV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            binding: decoder.field(1, DecodedPersistentId::decode)?,
            definition: decoder.field(2, DecodedCoreTypeDefinitionV1::decode)?,
            capability: decoder.field(3, DecodedCoreHirTypeCapabilityV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreTypeTargetSurfaceV1 {
    targets: Vec<CoreTypeTargetV1>,
}

impl CoreTypeTargetSurfaceV1 {
    pub fn from_core_export(export: &ExportHir) -> Result<Self, CoreTypeTargetSurfaceBuildError> {
        if export.cone != ConeIdentity::CORE {
            return Err(CoreTypeTargetSurfaceBuildError::NotCore(export.cone));
        }
        let nominals = source_nominals(export)?;
        let aliases = type_aliases(export)?;
        let mut targets = Vec::new();
        for binding in export.export_binding_identities.iter() {
            if binding.key().exporter() != ConeIdentity::CORE {
                return Err(CoreTypeTargetSurfaceBuildError::NonCoreBinding(
                    binding.id(),
                ));
            }
            let (definition, capability) = match binding.key().target() {
                BindableEntity::Type(id) => {
                    let exact = nominals
                        .get(&CoreTypeDefinitionV1::Type(id))
                        .copied()
                        .ok_or(CoreTypeTargetSurfaceBuildError::MissingNominal {
                            binding: binding.id(),
                            definition: CoreTypeDefinitionV1::Type(id),
                        })?
                        .into_exact()
                        .ok_or(CoreTypeTargetSurfaceBuildError::NominalCapabilityMismatch {
                            binding: binding.id(),
                        })?;
                    (
                        CoreTypeDefinitionV1::Type(id),
                        CoreHirTypeCapabilityV1::ParamFreeStrong(exact),
                    )
                }
                BindableEntity::GenericType(id) => {
                    let type_parameter_count = *nominals
                        .get(&CoreTypeDefinitionV1::GenericType(id))
                        .ok_or(CoreTypeTargetSurfaceBuildError::MissingNominal {
                            binding: binding.id(),
                            definition: CoreTypeDefinitionV1::GenericType(id),
                        })?;
                    (
                        CoreTypeDefinitionV1::GenericType(id),
                        CoreHirTypeCapabilityV1::GenericUnavailable {
                            type_parameter_count: type_parameter_count.into_generic().ok_or(
                                CoreTypeTargetSurfaceBuildError::NominalCapabilityMismatch {
                                    binding: binding.id(),
                                },
                            )?,
                        },
                    )
                }
                BindableEntity::TypeAlias(id) => {
                    let exact =
                        *aliases
                            .get(&id)
                            .ok_or(CoreTypeTargetSurfaceBuildError::MissingAlias {
                                binding: binding.id(),
                                alias: id,
                            })?;
                    let capability = if exact_root_is_nominal(export, exact) {
                        CoreHirTypeCapabilityV1::ParamFreeStrong(exact)
                    } else {
                        CoreHirTypeCapabilityV1::StructuralUnavailable(exact)
                    };
                    (CoreTypeDefinitionV1::TypeAlias(id), capability)
                }
                BindableEntity::ObjectValue(_)
                | BindableEntity::Function(_)
                | BindableEntity::GenericFunction(_)
                | BindableEntity::Property(_)
                | BindableEntity::ExtensionProperty(_)
                | BindableEntity::EnumVariant(_) => continue,
            };
            if export
                .export_definition_origins
                .get(definition.origin_subject())
                .is_none()
            {
                return Err(CoreTypeTargetSurfaceBuildError::MissingDefinitionOrigin {
                    binding: binding.id(),
                    definition,
                });
            }
            targets.push(CoreTypeTargetV1 {
                binding: binding.id(),
                definition,
                capability,
            });
        }
        Self::try_new(targets)
    }

    pub fn try_new(
        mut targets: Vec<CoreTypeTargetV1>,
    ) -> Result<Self, CoreTypeTargetSurfaceBuildError> {
        targets.sort_by_key(CoreTypeTargetV1::binding);
        if let Some(pair) = targets
            .windows(2)
            .find(|pair| pair[0].binding == pair[1].binding)
        {
            return Err(CoreTypeTargetSurfaceBuildError::DuplicateBinding(
                pair[0].binding,
            ));
        }
        Ok(Self { targets })
    }

    pub fn targets(&self) -> &[CoreTypeTargetV1] {
        &self.targets
    }
}

impl WireEncode for CoreTypeTargetSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.targets.len() as u64)?;
        for target in &self.targets {
            target.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedCoreTypeTargetSurfaceV1 {
    targets: Vec<DecodedCoreTypeTargetV1>,
}

impl DecodedCoreTypeTargetSurfaceV1 {
    pub fn validate(
        self,
        foundation: &ValidatedHirFoundation,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<CoreTypeTargetSurfaceV1, CoreTypeTargetSurfaceValidationError> {
        self.validate_against(foundation.canonical(), direct_surface)
    }

    fn validate_against(
        self,
        foundation: &CanonicalHirFoundation,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
    ) -> Result<CoreTypeTargetSurfaceV1, CoreTypeTargetSurfaceValidationError> {
        let mut expected = Vec::new();
        for binding in direct_surface.bindings() {
            let key = foundation.export_binding_key(*binding).ok_or(
                CoreTypeTargetSurfaceValidationError::UnknownDirectSurfaceBinding(*binding),
            )?;
            if matches!(
                key.target(),
                BindableEntity::Type(_)
                    | BindableEntity::GenericType(_)
                    | BindableEntity::TypeAlias(_)
            ) {
                expected.push(*binding);
            }
        }
        if self.targets.len() != expected.len() {
            return Err(CoreTypeTargetSurfaceValidationError::Coverage {
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
                .ok_or(CoreTypeTargetSurfaceValidationError::UnknownBinding {
                    index,
                    identity: *decoded.binding.as_array(),
                })?;
            if binding != expected_binding {
                return Err(CoreTypeTargetSurfaceValidationError::CoverageMismatch {
                    index,
                    expected: expected_binding,
                    actual: binding,
                });
            }
            targets.push(validate_target(foundation, binding, decoded)?);
        }
        Ok(CoreTypeTargetSurfaceV1 { targets })
    }
}

impl WireEncode for DecodedCoreTypeTargetSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.targets.len() as u64)?;
        for target in &self.targets {
            target.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCoreTypeTargetSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedCoreTypeTargetV1::decode(decoder))
            .map(|targets| Self { targets })
    }
}

#[derive(Clone, Copy)]
enum NominalCapability {
    Exact(PersistentExactTypeId),
    Generic(u32),
}

impl NominalCapability {
    const fn into_generic(self) -> Option<u32> {
        match self {
            Self::Generic(count) => Some(count),
            Self::Exact(_) => None,
        }
    }

    const fn into_exact(self) -> Option<PersistentExactTypeId> {
        match self {
            Self::Exact(exact) => Some(exact),
            Self::Generic(_) => None,
        }
    }
}

fn source_nominals(
    export: &ExportHir,
) -> Result<BTreeMap<CoreTypeDefinitionV1, NominalCapability>, CoreTypeTargetSurfaceBuildError> {
    let mut nominals = BTreeMap::new();
    for (id, declaration) in export.structs.iter() {
        insert_nominal(
            export,
            &mut nominals,
            &export.nominal_identities[id],
            export.struct_applications[declaration.self_application].canonical_type,
        )?;
    }
    for (id, declaration) in export.enums.iter() {
        insert_nominal(
            export,
            &mut nominals,
            &export.nominal_identities[id],
            export.enum_applications[declaration.self_application].canonical_type,
        )?;
    }
    for (id, declaration) in export.classes.iter() {
        insert_nominal(
            export,
            &mut nominals,
            &export.nominal_identities[id],
            export.class_applications[declaration.self_application].canonical_type,
        )?;
    }
    for (id, declaration) in export.interfaces.iter() {
        insert_nominal(
            export,
            &mut nominals,
            &export.nominal_identities[id],
            export.interface_applications[declaration.self_application].canonical_type,
        )?;
    }
    for (id, declaration) in export.objects.iter() {
        insert_nominal(
            export,
            &mut nominals,
            &export.nominal_identities[id],
            export.object_types[declaration.object_type].canonical_type,
        )?;
    }
    Ok(nominals)
}

fn insert_nominal(
    export: &ExportHir,
    nominals: &mut BTreeMap<CoreTypeDefinitionV1, NominalCapability>,
    identity: &HirNominalIdentity,
    self_type: crate::TypeId,
) -> Result<(), CoreTypeTargetSurfaceBuildError> {
    let Some(source) = identity.source() else {
        return Ok(());
    };
    if source.declaration().origin() != ConeIdentity::CORE {
        return Err(CoreTypeTargetSurfaceBuildError::NonCoreNominal);
    }
    let (definition, capability) = match source {
        HirSourceNominalIdentity::Concrete(record) => {
            let exact = export
                .type_identities
                .get(self_type)
                .and_then(crate::HirTypeIdentity::exact)
                .ok_or(CoreTypeTargetSurfaceBuildError::OpenConcreteNominal(
                    record.id(),
                ))?;
            if exact.key() != &ExactTypeKey::Nominal(record.id()) {
                return Err(CoreTypeTargetSurfaceBuildError::NominalExactMismatch(
                    record.id(),
                ));
            }
            (
                CoreTypeDefinitionV1::Type(record.id()),
                NominalCapability::Exact(exact.id()),
            )
        }
        HirSourceNominalIdentity::Generic(record) => {
            let count = record.key().duplicate_signature().type_parameter_count();
            if count == 0 {
                return Err(CoreTypeTargetSurfaceBuildError::EmptyGenericNominal(
                    record.id(),
                ));
            }
            (
                CoreTypeDefinitionV1::GenericType(record.id()),
                NominalCapability::Generic(count),
            )
        }
    };
    if nominals.insert(definition, capability).is_some() {
        return Err(CoreTypeTargetSurfaceBuildError::DuplicateNominal(
            definition,
        ));
    }
    Ok(())
}

fn type_aliases(
    export: &ExportHir,
) -> Result<BTreeMap<PersistentTypeAliasId, PersistentExactTypeId>, CoreTypeTargetSurfaceBuildError>
{
    let mut aliases = BTreeMap::new();
    for (alias, declaration) in export.type_aliases.iter() {
        let identity = &export.type_alias_identities[alias];
        if identity.key().origin() != ConeIdentity::CORE {
            return Err(CoreTypeTargetSurfaceBuildError::NonCoreAlias(identity.id()));
        }
        let exact = export
            .type_identities
            .get(declaration.target)
            .and_then(crate::HirTypeIdentity::exact)
            .ok_or(CoreTypeTargetSurfaceBuildError::OpenAliasTarget(
                identity.id(),
            ))?;
        if aliases.insert(identity.id(), exact.id()).is_some() {
            return Err(CoreTypeTargetSurfaceBuildError::DuplicateAlias(
                identity.id(),
            ));
        }
    }
    Ok(aliases)
}

fn exact_root_is_nominal(export: &ExportHir, exact: PersistentExactTypeId) -> bool {
    export.types.iter().any(|(ty, _)| {
        export.type_identities[ty].exact().is_some_and(|record| {
            record.id() == exact && matches!(record.key(), ExactTypeKey::Nominal(_))
        })
    })
}

fn validate_order(
    targets: &[DecodedCoreTypeTargetV1],
) -> Result<(), CoreTypeTargetSurfaceValidationError> {
    for (index, pair) in targets.windows(2).enumerate() {
        match pair[0].binding.as_array().cmp(pair[1].binding.as_array()) {
            std::cmp::Ordering::Equal => {
                return Err(CoreTypeTargetSurfaceValidationError::DuplicateBinding {
                    index: index + 1,
                    identity: *pair[1].binding.as_array(),
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(CoreTypeTargetSurfaceValidationError::NonCanonicalOrder {
                    index: index + 1,
                });
            }
            std::cmp::Ordering::Less => {}
        }
    }
    Ok(())
}

fn validate_target(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    decoded: DecodedCoreTypeTargetV1,
) -> Result<CoreTypeTargetV1, CoreTypeTargetSurfaceValidationError> {
    let key = foundation.export_binding_key(binding).ok_or(
        CoreTypeTargetSurfaceValidationError::UnknownBinding {
            index: 0,
            identity: *binding.as_array(),
        },
    )?;
    if key.exporter() != ConeIdentity::CORE {
        return Err(CoreTypeTargetSurfaceValidationError::NonCoreBinding(
            binding,
        ));
    }
    let (definition, source) = match decoded.definition {
        DecodedCoreTypeDefinitionV1::Type(decoded) => {
            let (id, source) = foundation.source_type_by_bytes(decoded.as_array()).ok_or(
                CoreTypeTargetSurfaceValidationError::UnknownType(*decoded.as_array()),
            )?;
            if key.target() != BindableEntity::Type(id) {
                return Err(CoreTypeTargetSurfaceValidationError::DefinitionMismatch(
                    binding,
                ));
            }
            (CoreTypeDefinitionV1::Type(id), source)
        }
        DecodedCoreTypeDefinitionV1::GenericType(decoded) => {
            let (id, source) = foundation.generic_type_by_bytes(decoded.as_array()).ok_or(
                CoreTypeTargetSurfaceValidationError::UnknownGenericType(*decoded.as_array()),
            )?;
            if key.target() != BindableEntity::GenericType(id) {
                return Err(CoreTypeTargetSurfaceValidationError::DefinitionMismatch(
                    binding,
                ));
            }
            (CoreTypeDefinitionV1::GenericType(id), source)
        }
        DecodedCoreTypeDefinitionV1::TypeAlias(decoded) => {
            let (id, source) = foundation.type_alias_by_bytes(decoded.as_array()).ok_or(
                CoreTypeTargetSurfaceValidationError::UnknownTypeAlias(*decoded.as_array()),
            )?;
            if key.target() != BindableEntity::TypeAlias(id) {
                return Err(CoreTypeTargetSurfaceValidationError::DefinitionMismatch(
                    binding,
                ));
            }
            (CoreTypeDefinitionV1::TypeAlias(id), source)
        }
    };
    if !super::direct_binding_matches_source(key, source) {
        return Err(CoreTypeTargetSurfaceValidationError::BindingSourceMismatch(
            binding,
        ));
    }
    if source.origin() != ConeIdentity::CORE {
        return Err(CoreTypeTargetSurfaceValidationError::NonCoreDefinition(
            definition,
        ));
    }
    if foundation
        .definition_origin(definition.origin_subject())
        .is_none()
    {
        return Err(CoreTypeTargetSurfaceValidationError::MissingDefinitionOrigin(definition));
    }
    let capability = validate_capability(
        foundation,
        binding,
        definition,
        source.duplicate_signature().type_parameter_count(),
        decoded.capability,
    )?;
    Ok(CoreTypeTargetV1 {
        binding,
        definition,
        capability,
    })
}

fn validate_capability(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    definition: CoreTypeDefinitionV1,
    type_parameter_count: u32,
    decoded: DecodedCoreHirTypeCapabilityV1,
) -> Result<CoreHirTypeCapabilityV1, CoreTypeTargetSurfaceValidationError> {
    match (definition, decoded) {
        (
            CoreTypeDefinitionV1::GenericType(_),
            DecodedCoreHirTypeCapabilityV1::GenericUnavailable {
                type_parameter_count: actual,
            },
        ) if actual > 0 && actual == type_parameter_count => {
            Ok(CoreHirTypeCapabilityV1::GenericUnavailable {
                type_parameter_count: actual,
            })
        }
        (
            CoreTypeDefinitionV1::Type(id),
            DecodedCoreHirTypeCapabilityV1::ParamFreeStrong(decoded),
        ) if type_parameter_count == 0 => {
            let (exact, key) = exact(foundation, decoded)?;
            if key != &ExactTypeKey::Nominal(id) {
                return Err(CoreTypeTargetSurfaceValidationError::ExactTargetMismatch(
                    binding,
                ));
            }
            Ok(CoreHirTypeCapabilityV1::ParamFreeStrong(exact))
        }
        (
            CoreTypeDefinitionV1::TypeAlias(_),
            DecodedCoreHirTypeCapabilityV1::ParamFreeStrong(decoded),
        ) if type_parameter_count == 0 => {
            let (exact, key) = exact(foundation, decoded)?;
            if !matches!(key, ExactTypeKey::Nominal(_)) {
                return Err(CoreTypeTargetSurfaceValidationError::CapabilityMismatch(
                    binding,
                ));
            }
            Ok(CoreHirTypeCapabilityV1::ParamFreeStrong(exact))
        }
        (
            CoreTypeDefinitionV1::TypeAlias(_),
            DecodedCoreHirTypeCapabilityV1::StructuralUnavailable(decoded),
        ) if type_parameter_count == 0 => {
            let (exact, key) = exact(foundation, decoded)?;
            if matches!(key, ExactTypeKey::Nominal(_)) {
                return Err(CoreTypeTargetSurfaceValidationError::CapabilityMismatch(
                    binding,
                ));
            }
            Ok(CoreHirTypeCapabilityV1::StructuralUnavailable(exact))
        }
        _ => Err(CoreTypeTargetSurfaceValidationError::CapabilityMismatch(
            binding,
        )),
    }
}

fn exact(
    foundation: &CanonicalHirFoundation,
    decoded: DecodedPersistentId<PersistentExactTypeId>,
) -> Result<(PersistentExactTypeId, &ExactTypeKey), CoreTypeTargetSurfaceValidationError> {
    foundation.exact_type_by_bytes(decoded.as_array()).ok_or(
        CoreTypeTargetSurfaceValidationError::UnknownExactType(*decoded.as_array()),
    )
}

#[derive(Debug)]
pub enum CoreTypeTargetSurfaceBuildError {
    NotCore(ConeIdentity),
    NonCoreBinding(PersistentExportBindingId),
    DuplicateBinding(PersistentExportBindingId),
    NonCoreNominal,
    OpenConcreteNominal(PersistentTypeId),
    NominalExactMismatch(PersistentTypeId),
    EmptyGenericNominal(PersistentGenericTypeId),
    DuplicateNominal(CoreTypeDefinitionV1),
    MissingNominal {
        binding: PersistentExportBindingId,
        definition: CoreTypeDefinitionV1,
    },
    NominalCapabilityMismatch {
        binding: PersistentExportBindingId,
    },
    NonCoreAlias(PersistentTypeAliasId),
    OpenAliasTarget(PersistentTypeAliasId),
    DuplicateAlias(PersistentTypeAliasId),
    MissingAlias {
        binding: PersistentExportBindingId,
        alias: PersistentTypeAliasId,
    },
    MissingDefinitionOrigin {
        binding: PersistentExportBindingId,
        definition: CoreTypeDefinitionV1,
    },
}

impl fmt::Display for CoreTypeTargetSurfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build core type target surface: {self:?}")
    }
}

impl std::error::Error for CoreTypeTargetSurfaceBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CoreTypeTargetSurfaceValidationError {
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
    UnknownType([u8; 32]),
    UnknownGenericType([u8; 32]),
    UnknownTypeAlias([u8; 32]),
    DefinitionMismatch(PersistentExportBindingId),
    NonCoreDefinition(CoreTypeDefinitionV1),
    MissingDefinitionOrigin(CoreTypeDefinitionV1),
    UnknownExactType([u8; 32]),
    ExactTargetMismatch(PersistentExportBindingId),
    CapabilityMismatch(PersistentExportBindingId),
}

impl fmt::Display for CoreTypeTargetSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid core type target surface: {self:?}")
    }
}

impl std::error::Error for CoreTypeTargetSurfaceValidationError {}

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
