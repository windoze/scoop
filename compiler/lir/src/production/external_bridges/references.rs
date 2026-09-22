use super::*;
use scoop_identity::{
    DecodedPersistentId, PersistentCallableBodyId, PersistentExactTypeId, PersistentSymbolKey,
};

impl StrongExternalLirBridgeSurfaceV1 {
    pub(crate) fn resolve_descriptor_reference(
        &self,
        provider: DecodedPersistentId<ConeIdentity>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Option<ExternalTypeDescriptor> {
        self.bridges.iter().find_map(|reference| match reference {
            StrongExternalLirBridgeV1::TypeDescriptor(descriptor)
                if descriptor.provider().as_array() == provider.as_array()
                    && descriptor.target().as_array() == exact.as_array() =>
            {
                Some(*descriptor)
            }
            _ => None,
        })
    }

    pub(crate) fn resolve_callable_reference(
        &self,
        provider: DecodedPersistentId<ConeIdentity>,
        body: DecodedPersistentId<PersistentCallableBodyId>,
    ) -> Option<(ConeIdentity, PersistentCallableBodyId)> {
        self.bridges.iter().find_map(|reference| match reference {
            StrongExternalLirBridgeV1::Callable(callable)
                if callable.provider().as_array() == provider.as_array() =>
            {
                match callable.bridge().expected_symbol().key() {
                    PersistentSymbolKey::CallableBody(candidate)
                        if candidate.as_array() == body.as_array() =>
                    {
                        Some((callable.provider(), candidate))
                    }
                    _ => None,
                }
            }
            _ => None,
        })
    }
}
