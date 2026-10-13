use super::{CapabilityId, CapabilityIdError, CapabilityRefinementError, ProfileName};
use scoop_wire::{Encoder, WireEncode};

const NAMESPACE: &str = "org.scoop-lang.target-profile";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetProfileId(ProfileName);

impl TargetProfileId {
    pub const fn new(name: &str, version: u32) -> Result<Self, CapabilityIdError> {
        match ProfileName::new(name, version) {
            Ok(name) => Ok(Self(name)),
            Err(error) => Err(error),
        }
    }
    pub fn canonical_name(&self) -> &str {
        self.0.name()
    }
    pub const fn major_version(self) -> u32 {
        self.0.version()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetProfileWireId(std::sync::Arc<TargetProfileIdentity>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct TargetProfileIdentity {
    id: TargetProfileId,
    capability: CapabilityId,
}

impl TargetProfileWireId {
    pub fn new(id: TargetProfileId) -> Self {
        Self(std::sync::Arc::new(TargetProfileIdentity {
            id,
            capability: CapabilityId::new(NAMESPACE, id.canonical_name(), id.major_version())
                .expect("typed target identity is canonical"),
        }))
    }
    pub fn id(&self) -> TargetProfileId {
        self.0.id
    }
    pub fn capability(&self) -> &CapabilityId {
        &self.0.capability
    }
    pub fn refine(capability: CapabilityId) -> Result<Self, CapabilityRefinementError> {
        if capability.namespace() != NAMESPACE {
            return Err(CapabilityRefinementError::namespace(NAMESPACE, capability));
        }
        let id = TargetProfileId::new(capability.name(), capability.major_version())
            .expect("validated capability has canonical name and version");
        Ok(Self(std::sync::Arc::new(TargetProfileIdentity {
            id,
            capability,
        })))
    }
}

impl WireEncode for TargetProfileWireId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.capability.encode(encoder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_identity_is_format_data_not_a_support_registry() {
        for (name, version) in [("darwin-aarch64", 2), ("future-target", 7)] {
            let capability = CapabilityId::new(NAMESPACE, name, version).unwrap();
            let identity = TargetProfileWireId::refine(capability.clone()).unwrap();
            assert_eq!(identity.capability(), &capability);
            assert_eq!(identity.id().canonical_name(), name);
            assert_eq!(identity.id().major_version(), version);
        }
        assert!(
            TargetProfileWireId::refine(
                CapabilityId::new("org.scoop-lang.backend-profile", "darwin-aarch64", 1).unwrap()
            )
            .is_err()
        );
        for (name, version) in [("", 1), ("Upper", 1), ("a.b", 1), ("llvm", 0)] {
            assert!(TargetProfileId::new(name, version).is_err());
        }
        assert!(TargetProfileId::new(&"a".repeat(63), 1).is_ok());
        assert!(TargetProfileId::new(&"a".repeat(64), 1).is_err());
    }
}
