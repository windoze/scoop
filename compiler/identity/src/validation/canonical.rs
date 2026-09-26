//! Identity queries over complete records produced by the current compilation.

use super::*;

impl PendingIdentityValidation {
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
        self.reserve_canonical_key_slot()?;
        self.insert_candidate(layer, id)?;
        let node = IdentityNode::trusted(id);
        self.canonical_keys
            .insert(CanonicalKeySlot::new::<I, K>(node.bytes), key);
        self.candidates
            .get_mut(&node)
            .expect("inserted identity")
            .resolved = true;
        Ok(())
    }
}
