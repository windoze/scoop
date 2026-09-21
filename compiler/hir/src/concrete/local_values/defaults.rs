use super::*;

/// Source values in a default's actual provider materialization. These do not
/// allocate runtime locals in the provider or describe call-site temporaries.
#[derive(Clone, Debug)]
pub struct DefaultLocalValueScope {
    pub owner: CallableMaterialization,
    pub values: Vec<DefaultLocalValueDefinition>,
}

#[derive(Clone, Debug)]
pub struct DefaultLocalValueDefinition {
    pub binding: BindingId,
    pub selector: LocalValueSelector,
    pub definition: crate::LocalValueDefinitionSite,
}

impl LocalValueIdentityBuilder<'_> {
    pub(super) fn collect_default_local_values(&mut self) -> Result<(), LocalValueIdentityError> {
        for (scope_index, scope) in self.inputs.default_local_values.iter().enumerate() {
            for (local_index, local) in scope.values.iter().enumerate() {
                let location = LocalValueLocation::DefaultLocal {
                    scope: scope_index as u32,
                    local: local_index as u32,
                };
                let key = LocalValueKey::new(scope.owner, local.selector.clone());
                let identity = if matches!(
                    local.selector,
                    LocalValueSelector::This | LocalValueSelector::Parameter { .. }
                ) && self.locations_by_key.contains_key(&key)
                {
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
                self.bind(scope.owner.context(), local.binding, identity);
            }
        }
        Ok(())
    }
}
