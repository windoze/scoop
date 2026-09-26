//! Identity queries over complete records produced by the current compilation.

use super::*;

impl PendingIdentityValidation {
    /// Continues collecting the later IR layers of an existing identity graph.
    pub fn from_graph(graph: ValidatedIdentityGraph) -> Self {
        Self {
            candidates: graph.candidates,
            canonical_keys: graph.canonical_keys,
            phase: ValidationPhase::Registering,
            resource_error: None,
            resource_path: WirePath::default(),
        }
    }

    pub fn register_canonical_callable_body(
        &mut self,
        layer: IdentityLayer,
        record: &RuntimeIdentityRecord<PersistentCallableBodyId>,
    ) -> Result<(), IdentityValidationError> {
        self.insert_canonical(layer, record.id(), record.shared_key())
    }

    pub fn register_canonical_c_abi_signature(
        &mut self,
        layer: IdentityLayer,
        record: &crate::CanonicalCAbiSignatureFingerprintRecord,
    ) -> Result<(), IdentityValidationError> {
        self.insert_canonical_leaf(layer, record.fingerprint())
    }

    pub fn register_canonical_c_abi_layout(
        &mut self,
        layer: IdentityLayer,
        record: &crate::CanonicalCAbiLayoutFingerprintRecord,
    ) -> Result<(), IdentityValidationError> {
        self.insert_canonical_leaf(layer, record.fingerprint())
    }

    pub fn register_canonical_native_contract(
        &mut self,
        layer: IdentityLayer,
        record: &crate::NativeExternalContractRecord,
    ) -> Result<(), IdentityValidationError> {
        self.insert_canonical_leaf(layer, record.fingerprint())
    }

    fn insert_canonical_leaf<I: PersistentId + 'static>(
        &mut self,
        layer: IdentityLayer,
        id: I,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let node = IdentityNode::trusted(id);
        if let Some(candidate) = self.candidates.get_mut(&node)
            && candidate.layer.is_none()
            && candidate.resolved
            && candidate.trusted_id.as_ref().downcast_ref::<I>() == Some(&id)
        {
            candidate.layer = Some(layer);
            return Ok(());
        }
        self.insert_candidate(layer, id)?;
        self.candidates
            .get_mut(&IdentityNode::trusted(id))
            .expect("inserted identity")
            .resolved = true;
        Ok(())
    }

    /// Retains an existing canonical record in its defining IR layer.
    pub fn register_canonical<I, K>(
        &mut self,
        layer: IdentityLayer,
        record: CborIdentityRecord<I, K>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Eq + Send + Sync + 'static,
    {
        self.insert_canonical(layer, record.id(), record.into_shared_key())
    }

    pub fn register_canonical_source_native_contract(
        &mut self,
        layer: IdentityLayer,
        record: &crate::SourceNativeExternalContractRecord,
    ) -> Result<(), IdentityValidationError> {
        self.insert_canonical(layer, record.id(), Arc::new(record.key()))
    }

    fn insert_canonical<I, K>(
        &mut self,
        layer: IdentityLayer,
        id: I,
        key: Arc<K>,
    ) -> Result<(), IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: Eq + Send + Sync + 'static,
    {
        self.require_registration_phase()?;
        let node = IdentityNode::trusted(id);
        let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
        if let Some(candidate) = self.candidates.get(&node)
            && candidate.layer.is_none()
            && candidate.resolved
        {
            let same_key = self.canonical_keys.get(&slot).is_some_and(|existing| {
                existing.as_any().downcast_ref::<K>() == Some(key.as_ref())
            });
            if !same_key {
                return self.fail(IdentityValidationError::IdentityCollision {
                    kind: node.kind,
                    id: node.bytes,
                });
            }
            self.candidates
                .get_mut(&node)
                .expect("existing identity")
                .layer = Some(layer);
            return Ok(());
        }
        self.reserve_canonical_key_slot()?;
        self.insert_candidate(layer, id)?;
        self.canonical_keys.insert(slot, key);
        self.candidates
            .get_mut(&node)
            .expect("inserted identity")
            .resolved = true;
        Ok(())
    }
}
