use super::{CallableAbiDecodeError, CallableAbiRecordV1};
use crate::{CallingConvention, ExternalCallableRootPlan};
use scoop_identity::{
    ConeIdentity, DecodedCanonicalScoopAbiFunctionSignature, DecodedPersistentId,
    DecodedPersistentSymbolRequest, DecodedStrongCallableDefinitionOwner, ObjectDefinitionPlanId,
    ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

#[derive(Clone, Debug)]
pub struct DecodedCallableAbiRecordV1 {
    target: DecodedStrongCallableDefinitionOwner,
    abi_signature: DecodedCanonicalScoopAbiFunctionSignature,
    expected_symbol: DecodedPersistentSymbolRequest,
    calling_convention: CallingConvention,
    root_plan: ExternalCallableRootPlan,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl WireEncode for DecodedCallableAbiRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        self.abi_signature.encode(encoder)?;
        encoder.field(3)?;
        self.expected_symbol.encode(encoder)?;
        encoder.field(4)?;
        self.calling_convention.encode(encoder)?;
        encoder.field(5)?;
        self.root_plan.encode(encoder)?;
        encoder.field(6)?;
        self.required_definition.encode(encoder)
    }
}

impl WireDecode for DecodedCallableAbiRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            target: decoder.field(1, DecodedStrongCallableDefinitionOwner::decode)?,
            abi_signature: decoder.field(2, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
            expected_symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            calling_convention: decoder.field(4, CallingConvention::decode)?,
            root_plan: decoder.field(5, ExternalCallableRootPlan::decode)?,
            required_definition: decoder.field(6, DecodedPersistentId::decode)?,
        })
    }
}

impl DecodedCallableAbiRecordV1 {
    pub fn validate(
        self,
        provider: ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CallableAbiRecordV1, CallableAbiDecodeError> {
        // Derived symbol and definition references need not be registered in a
        // consumer's local graph. Recompute them from the resolved target and
        // provider, then require the complete canonical payload to agree.
        let actual = encode(&self).map_err(CallableAbiDecodeError::Encode)?;
        let target = self
            .target
            .resolve(identities)
            .map_err(CallableAbiDecodeError::Target)?;
        let signature = self
            .abi_signature
            .resolve(identities)
            .map_err(CallableAbiDecodeError::Abi)?;
        let record = CallableAbiRecordV1::new(
            provider,
            target,
            signature,
            self.calling_convention,
            self.root_plan,
        )
        .map_err(CallableAbiDecodeError::Build)?;
        if encode(&record).map_err(CallableAbiDecodeError::Encode)? != actual {
            return Err(CallableAbiDecodeError::RecordMismatch);
        }
        Ok(record)
    }
}
