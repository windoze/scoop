//! Keep the independently replayed initialization ABI in the Strong reader state.

use scoop_wire::{WirePath, encode_canonical_temporary};

use super::*;

impl DecodedStrongProductionSectionV2 {
    pub fn validate_initialization_abi(
        self,
        expected: Option<Box<CallableAbiRecordV1>>,
    ) -> Result<
        InitializationAbiResolvedStrongProductionSectionV2,
        StrongInitializationAbiValidationError,
    > {
        let path = WirePath::root().field(11);
        let actual = encode_canonical_temporary(
            &InitializationAbi(self.initialization_cycle_abi.as_deref()),
            &path,
        )?;
        let canonical = encode_canonical_temporary(&InitializationAbi(expected.as_deref()), &path)?;

        if actual != canonical {
            return Err(StrongInitializationAbiValidationError::Mismatch);
        }
        Ok(InitializationAbiResolvedStrongProductionSectionV2 {
            external_bridges: self.external_bridges,
            canonical_definitions: self.canonical_definitions,
            object_definition_plans: self.object_definition_plans,
            digest_finalization_plan: self.digest_finalization_plan,
            registration_production: self.registration_production,
            image_plan: self.image_plan,
            entry_plan: self.entry_plan,
            shape_support_plan: self.shape_support_plan,
            generated_bridge_plan: self.generated_bridge_plan,
            initialization_cycle_abi: expected,
        })
    }
}

impl InitializationAbiResolvedStrongProductionSectionV2 {
    pub fn initialization_cycle_abi(&self) -> Option<&CallableAbiRecordV1> {
        self.initialization_cycle_abi.as_deref()
    }
}

struct InitializationAbi<'a, I>(Option<&'a I>);
impl<I: WireEncode> WireEncode for InitializationAbi<'_, I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        crate::encode_initialization_abi(self.0, encoder)
    }
}

#[derive(Debug)]
pub enum StrongInitializationAbiValidationError {
    Mismatch,
    Resource(WireError),
}
impl From<WireError> for StrongInitializationAbiValidationError {
    fn from(source: WireError) -> Self {
        Self::Resource(source)
    }
}
impl std::fmt::Display for StrongInitializationAbiValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid Strong initialization ABI: {self:?}")
    }
}
impl std::error::Error for StrongInitializationAbiValidationError {}
