use scoop_wire::{Encoder, WireEncode};

use super::Effect;
use crate::PersistentExactTypeId;

mod decode;

pub use decode::{
    DecodedExactCallableSignature, DecodedOptionalExactOwner, ExactCallableSignatureResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalExactOwner {
    Absent,
    Present(PersistentExactTypeId),
}

impl OptionalExactOwner {
    pub const fn from_option(owner: Option<PersistentExactTypeId>) -> Self {
        match owner {
            Some(owner) => Self::Present(owner),
            None => Self::Absent,
        }
    }

    pub const fn is_present(self) -> bool {
        matches!(self, Self::Present(_))
    }
}

impl WireEncode for OptionalExactOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Present(owner) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                owner.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactCallableSignature {
    effect: Effect,
    receiver: OptionalExactOwner,
    parameters: Vec<PersistentExactTypeId>,
    result: PersistentExactTypeId,
}

impl ExactCallableSignature {
    pub fn new(
        effect: Effect,
        receiver: Option<PersistentExactTypeId>,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    ) -> Self {
        Self {
            effect,
            receiver: OptionalExactOwner::from_option(receiver),
            parameters,
            result,
        }
    }

    pub const fn effect(&self) -> Effect {
        self.effect
    }

    pub const fn receiver(&self) -> OptionalExactOwner {
        self.receiver
    }

    pub fn parameters(&self) -> &[PersistentExactTypeId] {
        &self.parameters
    }

    pub const fn result(&self) -> PersistentExactTypeId {
        self.result
    }
}

impl WireEncode for ExactCallableSignature {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.effect.encode(encoder)?;
        encoder.field(2)?;
        self.receiver.encode(encoder)?;
        encoder.field(3)?;
        encoder.array(self.parameters.len() as u64)?;
        for parameter in &self.parameters {
            parameter.encode(encoder)?;
        }
        encoder.field(4)?;
        self.result.encode(encoder)
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::ExactCallableSignature;
    use crate::{ConeIdentity, Effect, PersistentExactTypeId};

    #[test]
    fn exact_signature_uses_explicit_absent_receiver() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![exact], exact);
        assert_eq!(
            hex(&encode(&signature).unwrap()),
            format!("a4010102a1000103815820{exact}045820{exact}")
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
