use super::*;
use crate::{DeclaredVisibilityV1, DecodedDeclarationAccessSourceV1};
use scoop_identity::PersistentTypeId;
use scoop_wire::WireErrorKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNestedNominalSupportV1 {
    ParamFree {
        inheritance_exact: DecodedPersistentId<PersistentExactTypeId>,
        representation_owner: DecodedPersistentId<PersistentTypeId>,
    },
    GenericTemplate,
}
impl WireDecode for DecodedNestedNominalSupportV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                wire::expect_fields(decoder, fields, 3)?;
                Ok(Self::ParamFree {
                    inheritance_exact: decoder.field(1, DecodedPersistentId::decode)?,
                    representation_owner: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            2 => {
                wire::expect_fields(decoder, fields, 1)?;
                Ok(Self::GenericTemplate)
            }
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for DecodedNestedNominalSupportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ParamFree {
                inheritance_exact,
                representation_owner,
            } => {
                wire::tag(encoder, 3, 1)?;
                encoder.field(1)?;
                inheritance_exact.encode(encoder)?;
                encoder.field(2)?;
                representation_owner.encode(encoder)
            }
            Self::GenericTemplate => wire::tag(encoder, 1, 2),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedNestedNominalPayloadV1 {
    source_nominal: DecodedSourceNominalId,
    source_interface: DecodedProtectedNestedSourceInterfaceV1,
    support: DecodedNestedNominalSupportV1,
}
impl DecodedProtectedNestedNominalPayloadV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedNestedNominalPayloadV1, NestedSourceResolutionError<E>> {
        self.resolve_at(resolver, meter, 1)
    }
    fn resolve_at<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<ProtectedNestedNominalPayloadV1, NestedSourceResolutionError<E>> {
        use NestedSourceResolutionError as Error;
        meter
            .check_semantic_depth(depth, &WirePath::root())
            .map_err(Error::Resource)?;
        let source_nominal = self
            .source_nominal
            .resolve(resolver)
            .map_err(Error::Foundation)?;
        let source_interface = self
            .source_interface
            .resolve_at(resolver, meter, depth + 1)?;
        let support = match self.support {
            DecodedNestedNominalSupportV1::ParamFree {
                inheritance_exact,
                representation_owner,
            } => NestedNominalSupportV1::ParamFree {
                inheritance_exact: resolver
                    .resolve(inheritance_exact)
                    .map_err(Error::Foundation)?,
                representation_owner: resolver
                    .resolve(representation_owner)
                    .map_err(Error::Foundation)?,
            },
            DecodedNestedNominalSupportV1::GenericTemplate => {
                NestedNominalSupportV1::GenericTemplate
            }
        };
        ProtectedNestedNominalPayloadV1::try_new(source_nominal, source_interface, support)
            .map_err(Error::Build)
    }
}
impl WireDecode for DecodedProtectedNestedNominalPayloadV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            source_nominal: decoder.field(1, DecodedSourceNominalId::decode)?,
            source_interface: decoder.field(2, DecodedProtectedNestedSourceInterfaceV1::decode)?,
            support: decoder.field(3, DecodedNestedNominalSupportV1::decode)?,
        })
    }
}
impl WireEncode for DecodedProtectedNestedNominalPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.source_nominal.encode(encoder)?;
        encoder.field(2)?;
        self.source_interface.encode(encoder)?;
        encoder.field(3)?;
        self.support.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSupportNestedInterfaceV1 {
    declaration: DecodedSourceNominalId,
    declaration_access: DecodedDeclarationAccessSourceV1,
    payload: DecodedProtectedNestedNominalPayloadV1,
}
impl DecodedNominalSupportNestedInterfaceV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<NominalSupportNestedInterfaceV1, NestedSourceResolutionError<E>> {
        self.resolve_at(resolver, meter, 1)
    }
    pub(super) fn resolve_at<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<NominalSupportNestedInterfaceV1, NestedSourceResolutionError<E>> {
        use NestedSourceResolutionError as Error;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(Error::Foundation)?;
        let access = self
            .declaration_access
            .resolve_metered(resolver, meter)
            .map_err(Error::Access)?;
        let payload = self.payload.resolve_at(resolver, meter, depth + 1)?;
        NominalSupportNestedInterfaceV1::try_new(declaration, access, payload).map_err(Error::Build)
    }
    pub(super) fn decode_fields(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        Ok(Self {
            declaration: decoder.field(1, DecodedSourceNominalId::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedProtectedNestedNominalPayloadV1::decode)?,
        })
    }
    pub(super) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_access.encode(encoder)?;
        encoder.field(3)?;
        self.payload.encode(encoder)
    }
}
impl WireDecode for DecodedNominalSupportNestedInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Self::decode_fields(decoder)
    }
}
impl WireEncode for DecodedNominalSupportNestedInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        self.encode_fields(encoder)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedNestedNominalInterfaceV1(DecodedNominalSupportNestedInterfaceV1);
impl DecodedProtectedNestedNominalInterfaceV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedNestedNominalInterfaceV1, NestedSourceResolutionError<E>> {
        let source = self.0.resolve(resolver, meter)?;
        if source.declaration_access().declared_visibility() != DeclaredVisibilityV1::Protected {
            return Err(NestedSourceResolutionError::Build(
                NestedSourceBuildError::Access,
            ));
        }
        Ok(ProtectedNestedNominalInterfaceV1 { source })
    }
}
impl WireDecode for DecodedProtectedNestedNominalInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        DecodedNominalSupportNestedInterfaceV1::decode(decoder).map(Self)
    }
}
impl WireEncode for DecodedProtectedNestedNominalInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}
