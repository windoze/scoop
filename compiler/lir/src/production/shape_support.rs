use scoop_identity::{
    DecodedPersistentId, DecodedPersistentSymbolRequest, ObjectDefinitionPlanId,
    PersistentExactTypeId, PersistentId, PersistentKeyResolver, PersistentLayoutId,
    PersistentScanId, PersistentSymbolRequest, PersistentTypeId, SourceDeclarationKey,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, encode};

use crate::{ConeLirFoundation, RegistrationIdentitySurfaceV1};

mod callable;
mod layout;
mod validation;
use validation::build_closure;
pub use validation::{ParamFreeShapeSupportBuildError, ParamFreeShapeSupportValidationError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedShapeSupportReasonV1 {
    ReferenceNominalRequiresNoBox,
}

impl WireEncode for ClosedShapeSupportReasonV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ReferenceNominalRequiresNoBox => 1,
        })
    }
}

impl WireDecode for ClosedShapeSupportReasonV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ReferenceNominalRequiresNoBox),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapeSupportAvailabilityV1<T> {
    Available(T),
    NotApplicable(ClosedShapeSupportReasonV1),
}

impl<T> ShapeSupportAvailabilityV1<T> {
    pub const fn available(&self) -> Option<&T> {
        match self {
            Self::Available(value) => Some(value),
            Self::NotApplicable(_) => None,
        }
    }
}

impl<T: WireEncode> WireEncode for ShapeSupportAvailabilityV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Available(value) => encode_value_sum(encoder, 1, value),
            Self::NotApplicable(reason) => encode_value_sum(encoder, 2, reason),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongShapeDefinitionV1<I: PersistentId> {
    semantic_id: I,
    definition_plan: ObjectDefinitionPlanId,
    symbol: PersistentSymbolRequest,
}

impl<I: PersistentId> StrongShapeDefinitionV1<I> {
    pub(crate) const fn from_artifact(
        semantic_id: I,
        definition_plan: ObjectDefinitionPlanId,
        symbol: PersistentSymbolRequest,
    ) -> Self {
        Self {
            semantic_id,
            definition_plan,
            symbol,
        }
    }

    pub const fn semantic_id(&self) -> I {
        self.semantic_id
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }
}

impl<I: PersistentId + WireEncode> WireEncode for StrongShapeDefinitionV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(3)?;
        self.symbol.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongShapeRegistrationV1<I: PersistentId> {
    semantic_id: I,
    definition_plan: ObjectDefinitionPlanId,
    symbol: PersistentSymbolRequest,
}

impl<I: PersistentId> StrongShapeRegistrationV1<I> {
    pub(crate) const fn from_artifact(
        semantic_id: I,
        definition_plan: ObjectDefinitionPlanId,
        symbol: PersistentSymbolRequest,
    ) -> Self {
        Self {
            semantic_id,
            definition_plan,
            symbol,
        }
    }

    pub const fn semantic_id(&self) -> I {
        self.semantic_id
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }
}

impl<I: PersistentId + WireEncode> WireEncode for StrongShapeRegistrationV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(3)?;
        self.symbol.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongExactShapeSupportV1 {
    nominal: PersistentTypeId,
    exact: PersistentExactTypeId,
    layout: StrongShapeDefinitionV1<PersistentLayoutId>,
    scan: StrongShapeDefinitionV1<PersistentScanId>,
    descriptor: StrongShapeDefinitionV1<PersistentExactTypeId>,
    registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
}

impl StrongExactShapeSupportV1 {
    pub(crate) const fn from_artifact(
        nominal: PersistentTypeId,
        exact: PersistentExactTypeId,
        layout: StrongShapeDefinitionV1<PersistentLayoutId>,
        scan: StrongShapeDefinitionV1<PersistentScanId>,
        descriptor: StrongShapeDefinitionV1<PersistentExactTypeId>,
        registration: StrongShapeRegistrationV1<PersistentExactTypeId>,
    ) -> Self {
        Self {
            nominal,
            exact,
            layout,
            scan,
            descriptor,
            registration,
        }
    }

    pub const fn nominal(&self) -> PersistentTypeId {
        self.nominal
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }

    pub const fn layout(&self) -> StrongShapeDefinitionV1<PersistentLayoutId> {
        self.layout
    }

    pub const fn scan(&self) -> StrongShapeDefinitionV1<PersistentScanId> {
        self.scan
    }

    pub const fn descriptor(&self) -> StrongShapeDefinitionV1<PersistentExactTypeId> {
        self.descriptor
    }

    pub const fn registration(&self) -> StrongShapeRegistrationV1<PersistentExactTypeId> {
        self.registration
    }
}

impl WireEncode for StrongExactShapeSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.nominal.encode(encoder)?;
        encoder.field(2)?;
        self.exact.encode(encoder)?;
        encoder.field(3)?;
        self.layout.encode(encoder)?;
        encoder.field(4)?;
        self.scan.encode(encoder)?;
        encoder.field(5)?;
        self.descriptor.encode(encoder)?;
        encoder.field(6)?;
        self.registration.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeShapeSupportRolesV1 {
    source_nominal: ShapeSupportAvailabilityV1<PersistentTypeId>,
    value_layout: ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentLayoutId>>,
    ref_scan: ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentScanId>>,
    type_descriptor: ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentExactTypeId>>,
    type_registration: ShapeSupportAvailabilityV1<StrongShapeRegistrationV1<PersistentExactTypeId>>,
    boxed_value: ShapeSupportAvailabilityV1<StrongExactShapeSupportV1>,
    coroutine_step: ShapeSupportAvailabilityV1<StrongExactShapeSupportV1>,
    coroutine_slot: ShapeSupportAvailabilityV1<StrongExactShapeSupportV1>,
}

pub(crate) struct ParamFreeShapeSupportRolePartsV1 {
    pub source_nominal: ShapeSupportAvailabilityV1<PersistentTypeId>,
    pub value_layout: ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentLayoutId>>,
    pub ref_scan: ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentScanId>>,
    pub type_descriptor: ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentExactTypeId>>,
    pub type_registration:
        ShapeSupportAvailabilityV1<StrongShapeRegistrationV1<PersistentExactTypeId>>,
    pub boxed_value: ShapeSupportAvailabilityV1<StrongExactShapeSupportV1>,
    pub coroutine_step: ShapeSupportAvailabilityV1<StrongExactShapeSupportV1>,
    pub coroutine_slot: ShapeSupportAvailabilityV1<StrongExactShapeSupportV1>,
}

impl ParamFreeShapeSupportRolesV1 {
    pub(crate) const fn from_artifact(parts: ParamFreeShapeSupportRolePartsV1) -> Self {
        Self {
            source_nominal: parts.source_nominal,
            value_layout: parts.value_layout,
            ref_scan: parts.ref_scan,
            type_descriptor: parts.type_descriptor,
            type_registration: parts.type_registration,
            boxed_value: parts.boxed_value,
            coroutine_step: parts.coroutine_step,
            coroutine_slot: parts.coroutine_slot,
        }
    }

    pub const fn source_nominal(&self) -> &ShapeSupportAvailabilityV1<PersistentTypeId> {
        &self.source_nominal
    }

    pub const fn value_layout(
        &self,
    ) -> &ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentLayoutId>> {
        &self.value_layout
    }

    pub const fn ref_scan(
        &self,
    ) -> &ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentScanId>> {
        &self.ref_scan
    }

    pub const fn type_descriptor(
        &self,
    ) -> &ShapeSupportAvailabilityV1<StrongShapeDefinitionV1<PersistentExactTypeId>> {
        &self.type_descriptor
    }

    pub const fn type_registration(
        &self,
    ) -> &ShapeSupportAvailabilityV1<StrongShapeRegistrationV1<PersistentExactTypeId>> {
        &self.type_registration
    }

    pub const fn boxed_value(&self) -> &ShapeSupportAvailabilityV1<StrongExactShapeSupportV1> {
        &self.boxed_value
    }

    pub const fn coroutine_step(&self) -> &ShapeSupportAvailabilityV1<StrongExactShapeSupportV1> {
        &self.coroutine_step
    }

    pub const fn coroutine_slot(&self) -> &ShapeSupportAvailabilityV1<StrongExactShapeSupportV1> {
        &self.coroutine_slot
    }
}

impl WireEncode for ParamFreeShapeSupportRolesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.source_nominal.encode(encoder)?;
        encoder.field(2)?;
        self.value_layout.encode(encoder)?;
        encoder.field(3)?;
        self.ref_scan.encode(encoder)?;
        encoder.field(4)?;
        self.type_descriptor.encode(encoder)?;
        encoder.field(5)?;
        self.type_registration.encode(encoder)?;
        encoder.field(6)?;
        self.boxed_value.encode(encoder)?;
        encoder.field(7)?;
        self.coroutine_step.encode(encoder)?;
        encoder.field(8)?;
        self.coroutine_slot.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeShapeSupportClosureV1 {
    owner: PersistentExactTypeId,
    root: scoop_identity::ConeIdentity,
    roles: ParamFreeShapeSupportRolesV1,
}

impl ParamFreeShapeSupportClosureV1 {
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }

    pub const fn root(&self) -> scoop_identity::ConeIdentity {
        self.root
    }

    pub const fn roles(&self) -> &ParamFreeShapeSupportRolesV1 {
        &self.roles
    }
}

impl WireEncode for ParamFreeShapeSupportClosureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.root.encode(encoder)?;
        encoder.field(3)?;
        self.roles.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeShapeSupportPlanSetV1 {
    closures: Vec<ParamFreeShapeSupportClosureV1>,
}

impl ParamFreeShapeSupportPlanSetV1 {
    pub fn from_sources<'source>(
        sources: impl IntoIterator<Item = &'source SourceDeclarationKey>,
        foundation: &ConeLirFoundation,
        registrations: &RegistrationIdentitySurfaceV1,
    ) -> Result<Self, ParamFreeShapeSupportBuildError> {
        let mut closures = sources
            .into_iter()
            .map(|source| build_closure(source, foundation, registrations))
            .collect::<Result<Vec<_>, _>>()?;
        closures.sort_unstable_by_key(ParamFreeShapeSupportClosureV1::owner);
        if let Some(pair) = closures
            .windows(2)
            .find(|pair| pair[0].owner == pair[1].owner)
        {
            return Err(ParamFreeShapeSupportBuildError::DuplicateOwner(
                pair[0].owner,
            ));
        }
        Ok(Self { closures })
    }

    pub fn closures(&self) -> &[ParamFreeShapeSupportClosureV1] {
        &self.closures
    }
}

impl WireEncode for ParamFreeShapeSupportPlanSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.closures)
    }
}

#[derive(Debug)]
pub(crate) struct DecodedStrongShapeDefinitionV1<I: PersistentId> {
    semantic_id: DecodedPersistentId<I>,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    symbol: DecodedPersistentSymbolRequest,
}

impl<I: PersistentId> WireEncode for DecodedStrongShapeDefinitionV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(3)?;
        self.symbol.encode(encoder)
    }
}

impl<I: PersistentId> WireDecode for DecodedStrongShapeDefinitionV1<I> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            semantic_id: decoder.field(1, DecodedPersistentId::decode)?,
            definition_plan: decoder.field(2, DecodedPersistentId::decode)?,
            symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct DecodedStrongShapeRegistrationV1<I: PersistentId> {
    semantic_id: DecodedPersistentId<I>,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    symbol: DecodedPersistentSymbolRequest,
}

impl<I: PersistentId> WireEncode for DecodedStrongShapeRegistrationV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(3)?;
        self.symbol.encode(encoder)
    }
}

impl<I: PersistentId> WireDecode for DecodedStrongShapeRegistrationV1<I> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            semantic_id: decoder.field(1, DecodedPersistentId::decode)?,
            definition_plan: decoder.field(2, DecodedPersistentId::decode)?,
            symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
        })
    }
}

#[derive(Debug)]
struct DecodedStrongExactShapeSupportV1 {
    nominal: DecodedPersistentId<PersistentTypeId>,
    exact: DecodedPersistentId<PersistentExactTypeId>,
    layout: DecodedStrongShapeDefinitionV1<PersistentLayoutId>,
    scan: DecodedStrongShapeDefinitionV1<PersistentScanId>,
    descriptor: DecodedStrongShapeDefinitionV1<PersistentExactTypeId>,
    registration: DecodedStrongShapeRegistrationV1<PersistentExactTypeId>,
}

impl WireEncode for DecodedStrongExactShapeSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.nominal.encode(encoder)?;
        encoder.field(2)?;
        self.exact.encode(encoder)?;
        encoder.field(3)?;
        self.layout.encode(encoder)?;
        encoder.field(4)?;
        self.scan.encode(encoder)?;
        encoder.field(5)?;
        self.descriptor.encode(encoder)?;
        encoder.field(6)?;
        self.registration.encode(encoder)
    }
}

impl WireDecode for DecodedStrongExactShapeSupportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            nominal: decoder.field(1, DecodedPersistentId::decode)?,
            exact: decoder.field(2, DecodedPersistentId::decode)?,
            layout: decoder.field(3, DecodedStrongShapeDefinitionV1::decode)?,
            scan: decoder.field(4, DecodedStrongShapeDefinitionV1::decode)?,
            descriptor: decoder.field(5, DecodedStrongShapeDefinitionV1::decode)?,
            registration: decoder.field(6, DecodedStrongShapeRegistrationV1::decode)?,
        })
    }
}

#[derive(Debug)]
enum DecodedShapeSupportAvailabilityV1<T> {
    Available(T),
    NotApplicable(ClosedShapeSupportReasonV1),
}

impl<T: WireEncode> WireEncode for DecodedShapeSupportAvailabilityV1<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Available(value) => encode_value_sum(encoder, 1, value),
            Self::NotApplicable(reason) => encode_value_sum(encoder, 2, reason),
        }
    }
}

impl<T: WireDecode> WireDecode for DecodedShapeSupportAvailabilityV1<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        decoder.field(1, |decoder| match tag {
            1 => T::decode(decoder).map(Self::Available),
            2 => ClosedShapeSupportReasonV1::decode(decoder).map(Self::NotApplicable),
            tag => Err(unknown_tag(decoder, tag)),
        })
    }
}

#[derive(Debug)]
pub(crate) struct DecodedParamFreeShapeSupportRolesV1 {
    source_nominal: DecodedShapeSupportAvailabilityV1<DecodedPersistentId<PersistentTypeId>>,
    value_layout:
        DecodedShapeSupportAvailabilityV1<DecodedStrongShapeDefinitionV1<PersistentLayoutId>>,
    ref_scan: DecodedShapeSupportAvailabilityV1<DecodedStrongShapeDefinitionV1<PersistentScanId>>,
    type_descriptor:
        DecodedShapeSupportAvailabilityV1<DecodedStrongShapeDefinitionV1<PersistentExactTypeId>>,
    type_registration:
        DecodedShapeSupportAvailabilityV1<DecodedStrongShapeRegistrationV1<PersistentExactTypeId>>,
    boxed_value: DecodedShapeSupportAvailabilityV1<DecodedStrongExactShapeSupportV1>,
    coroutine_step: DecodedShapeSupportAvailabilityV1<DecodedStrongExactShapeSupportV1>,
    coroutine_slot: DecodedShapeSupportAvailabilityV1<DecodedStrongExactShapeSupportV1>,
}

impl DecodedParamFreeShapeSupportRolesV1 {
    fn source_nominal(
        &self,
    ) -> Result<DecodedPersistentId<PersistentTypeId>, ParamFreeShapeSupportValidationError> {
        match self.source_nominal {
            DecodedShapeSupportAvailabilityV1::Available(source) => Ok(source),
            DecodedShapeSupportAvailabilityV1::NotApplicable(_) => {
                Err(ParamFreeShapeSupportValidationError::SourceNominalNotAvailable)
            }
        }
    }
}

impl WireEncode for DecodedParamFreeShapeSupportRolesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.source_nominal.encode(encoder)?;
        encoder.field(2)?;
        self.value_layout.encode(encoder)?;
        encoder.field(3)?;
        self.ref_scan.encode(encoder)?;
        encoder.field(4)?;
        self.type_descriptor.encode(encoder)?;
        encoder.field(5)?;
        self.type_registration.encode(encoder)?;
        encoder.field(6)?;
        self.boxed_value.encode(encoder)?;
        encoder.field(7)?;
        self.coroutine_step.encode(encoder)?;
        encoder.field(8)?;
        self.coroutine_slot.encode(encoder)
    }
}

impl WireDecode for DecodedParamFreeShapeSupportRolesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            source_nominal: decoder.field(1, DecodedShapeSupportAvailabilityV1::decode)?,
            value_layout: decoder.field(2, DecodedShapeSupportAvailabilityV1::decode)?,
            ref_scan: decoder.field(3, DecodedShapeSupportAvailabilityV1::decode)?,
            type_descriptor: decoder.field(4, DecodedShapeSupportAvailabilityV1::decode)?,
            type_registration: decoder.field(5, DecodedShapeSupportAvailabilityV1::decode)?,
            boxed_value: decoder.field(6, DecodedShapeSupportAvailabilityV1::decode)?,
            coroutine_step: decoder.field(7, DecodedShapeSupportAvailabilityV1::decode)?,
            coroutine_slot: decoder.field(8, DecodedShapeSupportAvailabilityV1::decode)?,
        })
    }
}

#[derive(Debug)]
struct DecodedParamFreeShapeSupportClosureV1 {
    owner: DecodedPersistentId<PersistentExactTypeId>,
    root: DecodedPersistentId<scoop_identity::ConeIdentity>,
    roles: DecodedParamFreeShapeSupportRolesV1,
}

impl WireEncode for DecodedParamFreeShapeSupportClosureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.root.encode(encoder)?;
        encoder.field(3)?;
        self.roles.encode(encoder)
    }
}

impl WireDecode for DecodedParamFreeShapeSupportClosureV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            root: decoder.field(2, DecodedPersistentId::decode)?,
            roles: decoder.field(3, DecodedParamFreeShapeSupportRolesV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedParamFreeShapeSupportPlanSetV1 {
    closures: Vec<DecodedParamFreeShapeSupportClosureV1>,
}

impl DecodedParamFreeShapeSupportPlanSetV1 {
    pub fn validate<'source>(
        self,
        sources: impl IntoIterator<Item = &'source SourceDeclarationKey>,
        identities: &mut ValidatedIdentityGraph,
        foundation: &ConeLirFoundation,
        registrations: &RegistrationIdentitySurfaceV1,
    ) -> Result<ParamFreeShapeSupportPlanSetV1, ParamFreeShapeSupportValidationError> {
        let actual = encode(&self).map_err(ParamFreeShapeSupportValidationError::Encode)?;
        for closure in &self.closures {
            closure
                .root
                .verify(foundation.producer())
                .map_err(|_| ParamFreeShapeSupportValidationError::WrongRoot)?;
            let source = closure.roles.source_nominal()?;
            let key = identities
                .resolve_key(source)
                .map_err(ParamFreeShapeSupportValidationError::Identity)?;
            let _: std::sync::Arc<SourceDeclarationKey> = key;
        }
        let expected =
            ParamFreeShapeSupportPlanSetV1::from_sources(sources, foundation, registrations)
                .map_err(ParamFreeShapeSupportValidationError::Expected)?;
        let expected_bytes =
            encode(&expected).map_err(ParamFreeShapeSupportValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(ParamFreeShapeSupportValidationError::PlanMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedParamFreeShapeSupportPlanSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.closures)
    }
}

impl WireDecode for DecodedParamFreeShapeSupportPlanSetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedParamFreeShapeSupportClosureV1::decode(decoder))
            .map(|closures| Self { closures })
    }
}

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

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

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

mod link;
#[cfg(test)]
mod tests;
