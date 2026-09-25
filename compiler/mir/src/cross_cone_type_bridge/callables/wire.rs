use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedParamFreeMirCallableBindingV1 {
    origin: DecodedMirCallableOriginV1,
    implementation: DecodedStrongCallableDefinitionOwner,
    semantic: DecodedMirBridgeCallableSignatureV1,
    lowered: DecodedMirBridgeCallableSignatureV1,
    role: DecodedMirCallableLoweringRoleV1,
}
impl DecodedParamFreeMirCallableBindingV1 {
    pub fn validate(
        self,
        graph: &mut ValidatedIdentityGraph,
        foundation: &crate::OdrFreeMirFoundation,
        types: &dyn MirTypeBridgeTypeLookupV1,
    ) -> Result<ParamFreeMirCallableBindingV1, MirCallableBridgeError> {
        let origin = self.origin.resolve(graph)?;
        let implementation = self.implementation.resolve(graph)?;
        let semantic = self.semantic.resolve(graph)?;
        let lowered = self.lowered.resolve(graph)?;
        let role = self.role.resolve(graph)?;

        ParamFreeMirCallableBindingV1::try_new(
            MirCallableBridgeAuthority {
                identities: graph,
                foundation,
                types,
            },
            origin,
            implementation,
            semantic,
            lowered,
            role,
        )
    }
}
macro_rules! encode_binding {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(5)?;
                encoder.field(1)?;
                self.origin.encode(encoder)?;
                encoder.field(2)?;
                self.implementation.encode(encoder)?;
                encoder.field(3)?;
                self.semantic.encode(encoder)?;
                encoder.field(4)?;
                self.lowered.encode(encoder)?;
                encoder.field(5)?;
                self.role.encode(encoder)
            }
        }
    };
}
encode_binding!(ParamFreeMirCallableBindingV1);
encode_binding!(DecodedParamFreeMirCallableBindingV1);
impl WireDecode for DecodedParamFreeMirCallableBindingV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            origin: decoder.field(1, DecodedMirCallableOriginV1::decode)?,
            implementation: decoder.field(2, DecodedStrongCallableDefinitionOwner::decode)?,
            semantic: decoder.field(3, DecodedMirBridgeCallableSignatureV1::decode)?,
            lowered: decoder.field(4, DecodedMirBridgeCallableSignatureV1::decode)?,
            role: decoder.field(5, DecodedMirCallableLoweringRoleV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalMirCallableBindingsV1 {
    entries: Vec<DecodedParamFreeMirCallableBindingV1>,
}
impl DecodedCanonicalMirCallableBindingsV1 {
    pub fn validate(
        self,
        graph: &mut ValidatedIdentityGraph,
        foundation: &crate::OdrFreeMirFoundation,
        types: &dyn MirTypeBridgeTypeLookupV1,
    ) -> Result<CanonicalMirCallableBindingsV1, MirCallableBridgeError> {
        let path = WirePath::root();
        let mut entries: Vec<ParamFreeMirCallableBindingV1> = Vec::new();
        scoop_wire::allocation::try_reserve(&mut entries, self.entries.len(), &path)
            .map_err(MirCallableBridgeError::Resource)?;
        for (index, decoded) in self.entries.into_iter().enumerate() {
            let entry = decoded.validate(graph, foundation, types)?;
            if entries
                .last()
                .is_some_and(|previous| previous.implementation() >= entry.implementation())
            {
                return Err(MirCallableBridgeError::NonCanonicalBindingOrder { index });
            }
            entries.push(entry);
        }
        Ok(CanonicalMirCallableBindingsV1 { entries })
    }
}
impl WireEncode for CanonicalMirCallableBindingsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.entries)
    }
}
impl WireEncode for DecodedCanonicalMirCallableBindingsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.entries)
    }
}
impl WireDecode for DecodedCanonicalMirCallableBindingsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedParamFreeMirCallableBindingV1::decode(decoder))
            .map(|entries| Self { entries })
    }
}
