use super::*;
use crate::cross_cone_type_semantics::wire as transport;
use crate::{
    DecodedExportDefinitionSourceV1, DecodedSourceParameterShapeV1,
    MeteredInterfaceResolutionError, ProtectedCallableInterfaceResolver,
};
use scoop_identity::DecodedCallableTemplateOrigin;
use scoop_wire::{Decoder, WireDecode, WireErrorKind};

mod records;

fn tag(kind: ProtectedParameterCallingKindV1) -> u8 {
    match kind {
        ProtectedParameterCallingKindV1::Required => 1,
        ProtectedParameterCallingKindV1::Default => 2,
        ProtectedParameterCallingKindV1::VarargEmpty => 3,
        ProtectedParameterCallingKindV1::VarargDefault => 4,
    }
}
fn calling(decoder: &mut Decoder<'_, '_>) -> Result<ProtectedParameterCallingKindV1, WireError> {
    Ok(match decoder.unsigned()? {
        1 => ProtectedParameterCallingKindV1::Required,
        2 => ProtectedParameterCallingKindV1::Default,
        3 => ProtectedParameterCallingKindV1::VarargEmpty,
        4 => ProtectedParameterCallingKindV1::VarargDefault,
        tag => {
            return Err(transport::error(decoder, WireErrorKind::UnknownTag { tag }));
        }
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedParameter {
    shape: DecodedSourceParameterShapeV1,
    calling: ProtectedParameterCallingKindV1,
    origin: DecodedExportDefinitionSourceV1,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedProtocol {
    owner: DecodedCallableTemplateOrigin,
    parameters: Vec<DecodedParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceSourceParameterProtocolsV1 {
    records: Vec<DecodedProtocol>,
}
impl DecodedCanonicalInheritanceSourceParameterProtocolsV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E: fmt::Display>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalInheritanceSourceParameterProtocolsV1, SourceInventoryError> {
        let mut records = reserve(self.records.len(), meter)?;
        for record in self.records {
            let path = WirePath::root();
            meter.charge_work(64, &path)?;
            let owner = record.owner.resolve(resolver).map_err(reference)?;
            let mut parameters = reserve(record.parameters.len(), meter)?;
            for parameter in record.parameters {
                let shape = parameter
                    .shape
                    .resolve_metered(resolver, meter)
                    .map_err(|error| match error {
                        MeteredInterfaceResolutionError::Resource(e) => {
                            SourceInventoryError::Resource(e)
                        }
                        MeteredInterfaceResolutionError::Value(e) => reference(e),
                    })?;
                parameter.origin.charge_resolution_at(meter, &path, 3)?;
                let origin = parameter.origin.resolve(resolver).map_err(reference)?;
                parameters.push(InheritanceSourceParameterV1::new(
                    shape,
                    parameter.calling,
                    origin,
                ));
            }
            records.push(InheritanceSourceParameterProtocolV1::try_new(
                owner, parameters, meter,
            )?);
        }
        CanonicalInheritanceSourceParameterProtocolsV1::from_ordered(records, meter)
    }
}
impl WireDecode for DecodedCanonicalInheritanceSourceParameterProtocolsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedProtocol::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalInheritanceSourceParameterProtocolsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        transport::sequence(encoder, &self.records)
    }
}
