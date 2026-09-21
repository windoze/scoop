use super::*;

impl LocalValueIdentityBuilder<'_> {
    pub(super) fn collect_callable_references(
        &mut self,
    ) -> Result<Vec<CallableReferenceLocalValue>, LocalValueIdentityError> {
        let mut output = Vec::with_capacity(self.inputs.callable_references.len());
        let mut receivers = BTreeMap::<
            LocalValueKey,
            (
                PersistentLocalValueId,
                crate::DefinitionOrigin,
                LocalValueLocation,
            ),
        >::new();
        for (reference_id, reference) in self.inputs.callable_references.iter() {
            let value = match &reference.target {
                CallableReferenceTarget::Named(_) | CallableReferenceTarget::Local { .. } => {
                    CallableReferenceLocalValue::Unbound
                }
                CallableReferenceTarget::BoundMember { .. }
                | CallableReferenceTarget::BoundExtension { .. } => {
                    let location = LocalValueLocation::CallableReferenceReceiver {
                        reference: raw_arena_index(reference_id),
                    };
                    let owner = *reference.identity.materialization();
                    let selector = LocalValueSelector::BoundReceiver {
                        path: reference.identity.definition_path().clone(),
                    };
                    let key = LocalValueKey::new(owner, selector.clone());
                    let identity = if let Some((identity, origin, first)) = receivers.get(&key) {
                        if *origin != reference.origin {
                            return Err(LocalValueIdentityError::DuplicateSelector {
                                first: *first,
                                second: location,
                            });
                        }
                        *identity
                    } else {
                        let identity =
                            self.record_source(owner, selector, reference.origin, location)?;
                        receivers.insert(key, (identity, reference.origin, location));
                        identity
                    };
                    CallableReferenceLocalValue::Bound(identity)
                }
            };
            output.push(value);
        }
        Ok(output)
    }
}
