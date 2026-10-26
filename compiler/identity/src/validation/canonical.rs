//! Identity queries over complete records produced by the current compilation.

use super::*;

impl PendingIdentityValidation {
    /// Continues collecting the later IR layers of an existing identity graph.
    pub fn from_graph(graph: ValidatedIdentityGraph) -> Self {
        Self {
            candidates: Rc::unwrap_or_clone(graph.candidates),
            declared: graph.declared.to_vec(),
            checking: Vec::new(),
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
            return self.record_declaration(node);
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

    fn record_declaration(&mut self, node: IdentityNode) -> Result<(), IdentityValidationError> {
        scoop_wire::allocation::try_reserve(&mut self.declared, 1, &self.resource_path)
            .map_err(|error| self.poison(IdentityValidationError::Resource(error)))?;
        self.declared.push(node);
        Ok(())
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
        if let Some(candidate) = self.candidates.get(&node)
            && candidate.layer.is_none()
            && candidate.resolved
        {
            let same_key = candidate.key_ref::<I, K>() == Some(key.as_ref());
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
            return self.record_declaration(node);
        }
        self.insert_candidate(layer, id)?;
        let candidate = self.candidates.get_mut(&node).expect("inserted identity");
        candidate.canonical_key = Some(key);
        candidate.resolved = true;
        Ok(())
    }
}
