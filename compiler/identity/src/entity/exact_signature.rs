use scoop_wire::{Encoder, WireEncodeV1};

use super::EffectV1;
use crate::PersistentExactTypeId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionalExactOwnerV1 {
    Absent,
    Present(PersistentExactTypeId),
}

impl OptionalExactOwnerV1 {
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

impl WireEncodeV1 for OptionalExactOwnerV1 {
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
pub struct ExactCallableSignatureV1 {
    effect: EffectV1,
    receiver: OptionalExactOwnerV1,
    parameters: Vec<PersistentExactTypeId>,
    result: PersistentExactTypeId,
}

impl ExactCallableSignatureV1 {
    pub fn new(
        effect: EffectV1,
        receiver: Option<PersistentExactTypeId>,
        parameters: Vec<PersistentExactTypeId>,
        result: PersistentExactTypeId,
    ) -> Self {
        Self {
            effect,
            receiver: OptionalExactOwnerV1::from_option(receiver),
            parameters,
            result,
        }
    }

    pub const fn effect(&self) -> EffectV1 {
        self.effect
    }

    pub const fn receiver(&self) -> OptionalExactOwnerV1 {
        self.receiver
    }

    pub fn parameters(&self) -> &[PersistentExactTypeId] {
        &self.parameters
    }

    pub const fn result(&self) -> PersistentExactTypeId {
        self.result
    }
}

impl WireEncodeV1 for ExactCallableSignatureV1 {
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

    use super::ExactCallableSignatureV1;
    use crate::{ConeIdentity, EffectV1, PersistentExactTypeId};

    #[test]
    fn exact_signature_uses_explicit_absent_receiver() {
        let exact = PersistentExactTypeId(ConeIdentity::CORE.0);
        let signature = ExactCallableSignatureV1::new(EffectV1::Ordinary, None, vec![exact], exact);
        assert_eq!(
            hex(&encode(&signature).unwrap()),
            format!("a4010102a1000103815820{exact}045820{exact}")
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
