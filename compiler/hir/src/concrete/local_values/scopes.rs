use super::*;

/// Source values retained in their original lexical materialization. Defaults
/// and shared initialization may store copies in a different executable body.
#[derive(Clone, Debug)]
pub struct LexicalLocalValueScope {
    pub owner: CallableMaterialization,
    pub values: Vec<LexicalLocalValueDefinition>,
}

#[derive(Clone, Debug)]
pub struct LexicalLocalValueDefinition {
    pub binding: BindingId,
    pub selector: LocalValueSelector,
    pub definition: crate::LocalValueDefinitionSite,
}

impl LocalValueIdentityBuilder<'_> {
    pub(super) fn collect_lexical_local_values(&mut self) -> Result<(), LocalValueIdentityError> {
        for (scope_index, scope) in self.inputs.lexical_local_values.iter().enumerate() {
            for (local_index, local) in scope.values.iter().enumerate() {
                let location = LocalValueLocation::LexicalLocal {
                    scope: scope_index as u32,
                    local: local_index as u32,
                };
                let key = LocalValueKey::new(scope.owner, local.selector.clone());
                let identity = if self.locations_by_key.contains_key(&key) {
                    CborIdentityRecord::<PersistentLocalValueId, _>::from_key(key)
                        .map_err(|error| LocalValueIdentityError::Hash {
                            location,
                            reason: error.to_string(),
                        })?
                        .id()
                } else {
                    self.record(
                        scope.owner,
                        local.selector.clone(),
                        &local.definition,
                        location,
                    )?
                };
                self.values_by_binding
                    .entry(BindingKey {
                        context: scope.owner.context(),
                        binding: local.binding,
                    })
                    .or_default()
                    .push(LocalValueBinding::Lexical(identity));
            }
        }
        Ok(())
    }
}
