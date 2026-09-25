use super::*;
use crate::ProtectedParameterCallingKindV1;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{Encoder, WireEncode};
use std::collections::BTreeSet;

mod wire;
pub use wire::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalSourceParameterProtocolV1 {
    owner: CallableTemplateOrigin,
    parameters: Vec<InheritanceSourceParameterV1>,
}
impl NominalSourceParameterProtocolV1 {
    pub fn try_new(
        owner: CallableTemplateOrigin,
        parameters: Vec<InheritanceSourceParameterV1>,
    ) -> Result<Self, SourceInventoryError> {
        if !matches!(
            owner,
            CallableTemplateOrigin::Function(_)
                | CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Constructor(_)
                | CallableTemplateOrigin::VariantConstructor(_)
        ) {
            return Err(reference(
                "nominal parameter owner must be a source function, constructor or variant",
            ));
        }

        let mut names = BTreeSet::new();
        let mut vararg = false;
        for parameter in &parameters {
            let name = parameter.shape().name();

            if !names.insert(name) {
                return Err(reference("duplicate nominal source parameter name"));
            }
            if matches!(
                parameter.calling_kind(),
                ProtectedParameterCallingKindV1::VarargEmpty
                    | ProtectedParameterCallingKindV1::VarargDefault
            ) && std::mem::replace(&mut vararg, true)
            {
                return Err(reference("multiple nominal source vararg parameters"));
            }
        }
        Ok(Self { owner, parameters })
    }
    pub(crate) fn into_parts(self) -> (CallableTemplateOrigin, Vec<InheritanceSourceParameterV1>) {
        (self.owner, self.parameters)
    }
    pub const fn owner(&self) -> CallableTemplateOrigin {
        self.owner
    }
    pub fn parameters(&self) -> &[InheritanceSourceParameterV1] {
        &self.parameters
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalNominalSourceParameterProtocolsV1 {
    records: Vec<NominalSourceParameterProtocolV1>,
}
impl CanonicalNominalSourceParameterProtocolsV1 {
    pub fn try_new(
        mut records: Vec<NominalSourceParameterProtocolV1>,
    ) -> Result<Self, SourceInventoryError> {
        records.sort_unstable_by_key(NominalSourceParameterProtocolV1::owner);
        Self::from_ordered(records)
    }
    fn from_ordered(
        records: Vec<NominalSourceParameterProtocolV1>,
    ) -> Result<Self, SourceInventoryError> {
        validate_order(
            &records,
            NominalSourceParameterProtocolV1::owner,
            "nominal source parameter protocols",
        )?;
        Ok(Self { records })
    }
    pub fn records(&self) -> &[NominalSourceParameterProtocolV1] {
        &self.records
    }
    pub fn get(&self, owner: CallableTemplateOrigin) -> Option<&NominalSourceParameterProtocolV1> {
        self.records
            .binary_search_by_key(&owner, NominalSourceParameterProtocolV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}
impl WireEncode for CanonicalNominalSourceParameterProtocolsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::wire::sequence(encoder, &self.records)
    }
}

impl WireEncode for NominalSourceParameterProtocolV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        super::wire::sequence(encoder, &self.parameters)
    }
}
