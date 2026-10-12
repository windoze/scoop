use super::{CapabilityId, CapabilityIdError, CapabilityRefinementError, ProfileName};
use scoop_wire::{Encoder, WireEncode};

const NAMESPACE: &str = "org.scoop-lang.backend-profile";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendProfileId(ProfileName);

impl BackendProfileId {
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
pub struct BackendProfileWireId {
    id: BackendProfileId,
    capability: CapabilityId,
}

impl BackendProfileWireId {
    pub fn new(id: BackendProfileId) -> Self {
        Self {
            id,
            capability: CapabilityId::new(NAMESPACE, id.canonical_name(), id.major_version())
                .expect("typed backend identity is canonical"),
        }
    }
    pub const fn id(&self) -> BackendProfileId {
        self.id
    }
    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }
    pub fn refine(capability: CapabilityId) -> Result<Self, CapabilityRefinementError> {
        if capability.namespace() != NAMESPACE {
            return Err(CapabilityRefinementError::namespace(NAMESPACE, capability));
        }
        let id = BackendProfileId::new(capability.name(), capability.major_version())
            .expect("validated capability has canonical name and version");
        Ok(Self { id, capability })
    }
    pub fn llvm_22_1() -> Self {
        Self::new(BackendProfileId::new("llvm-22-1", 1).unwrap())
    }
    pub fn llvm_22_1_linux_x86_64() -> Self {
        Self::new(BackendProfileId::new("llvm-22-1-linux-x86-64", 1).unwrap())
    }
}

impl WireEncode for BackendProfileWireId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.capability.encode(encoder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backend_identity_is_format_data_not_a_support_registry() {
        for (name, version) in [("llvm-22-1", 2), ("future-backend", 7)] {
            let capability = CapabilityId::new(NAMESPACE, name, version).unwrap();
            let identity = BackendProfileWireId::refine(capability.clone()).unwrap();
            assert_eq!(identity.capability(), &capability);
            assert_eq!(identity.id().canonical_name(), name);
            assert_eq!(identity.id().major_version(), version);
        }
        assert!(
            BackendProfileWireId::refine(
                CapabilityId::new("org.scoop-lang.target-profile", "llvm-22-1", 1).unwrap()
            )
            .is_err()
        );
        for (name, version) in [("", 1), ("Upper", 1), ("a.b", 1), ("llvm", 0)] {
            assert!(BackendProfileId::new(name, version).is_err());
        }
        assert!(BackendProfileId::new(&"a".repeat(63), 1).is_ok());
        assert!(BackendProfileId::new(&"a".repeat(64), 1).is_err());
    }
}
