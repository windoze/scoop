use super::{CapabilityId, CapabilityIdError, CapabilityRefinementError};
use scoop_wire::{Encoder, WireEncode};

const NAMESPACE: &str = "org.scoop-lang.c-bridge-toolchain-profile";

/// Canonical producer identity; execution support belongs to the toolchain.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CBridgeToolchainProfileId(CapabilityId);

impl CBridgeToolchainProfileId {
    pub fn new(name: &str, version: u32) -> Result<Self, CapabilityIdError> {
        CapabilityId::new(NAMESPACE, name, version).map(Self)
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.0
    }

    pub fn refine(capability: CapabilityId) -> Result<Self, CapabilityRefinementError> {
        if capability.namespace() != NAMESPACE {
            return Err(CapabilityRefinementError::namespace(NAMESPACE, capability));
        }
        Ok(Self(capability))
    }
}

impl WireEncode for CBridgeToolchainProfileId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_producer_identity_does_not_imply_execution_support() {
        for (name, version) in [("unknown-producer", 1), ("darwin-aarch64-apple-clang", 2)] {
            let capability = CapabilityId::new(NAMESPACE, name, version).unwrap();
            assert_eq!(
                CBridgeToolchainProfileId::refine(capability.clone())
                    .unwrap()
                    .capability(),
                &capability
            );
        }
        assert!(CBridgeToolchainProfileId::new("Upper", 1).is_err());
        assert!(CBridgeToolchainProfileId::new("compiler", 0).is_err());
        assert!(
            CBridgeToolchainProfileId::refine(
                CapabilityId::new("org.scoop-lang.target-profile", "compiler", 1).unwrap()
            )
            .is_err()
        );
    }
}
