//! Complete selected callable data shared by every external MIR use.

use crate::{GcEffect, ParamFreeMirCallableBindingV1, SelectedDependencyMirCallableV1};
use scoop_identity::{ConeIdentity, ExactCallableSignature, StrongCallableDefinitionOwner};

#[derive(Clone, Debug, Eq, PartialEq)]
enum SelectedCallableDefinition {
    Direct(SelectedDependencyMirCallableV1),
    Lowered {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
        definition: Box<ParamFreeMirCallableBindingV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedExternalMirCallable {
    definition: SelectedCallableDefinition,
}

impl SelectedExternalMirCallable {
    pub fn dependency(record: SelectedDependencyMirCallableV1) -> Self {
        Self {
            definition: SelectedCallableDefinition::Direct(record),
        }
    }

    /// Retains the provider's complete source and physical lowering signatures.
    pub fn from_lowered(
        provider: ConeIdentity,
        definition: ParamFreeMirCallableBindingV1,
    ) -> Result<Self, crate::MirCallableBridgeError> {
        let scoop_identity::CallableDefinitionOwner::Strong(target) = definition.implementation()
        else {
            return Err(crate::MirCallableBridgeError::RoleMismatch);
        };
        Ok(Self {
            definition: SelectedCallableDefinition::Lowered {
                provider,
                target,
                definition: Box::new(definition),
            },
        })
    }

    pub const fn direct_record(&self) -> Option<&SelectedDependencyMirCallableV1> {
        match &self.definition {
            SelectedCallableDefinition::Direct(record) => Some(record),
            SelectedCallableDefinition::Lowered { .. } => None,
        }
    }

    pub const fn provider(&self) -> ConeIdentity {
        match &self.definition {
            SelectedCallableDefinition::Direct(record) => record.provider(),
            SelectedCallableDefinition::Lowered { provider, .. } => *provider,
        }
    }

    pub fn implementation(&self) -> StrongCallableDefinitionOwner {
        match &self.definition {
            SelectedCallableDefinition::Direct(record) => record.implementation(),
            SelectedCallableDefinition::Lowered { target, .. } => *target,
        }
    }

    pub fn signature(&self) -> &ExactCallableSignature {
        match &self.definition {
            SelectedCallableDefinition::Direct(record) => record.signature(),
            SelectedCallableDefinition::Lowered { definition, .. } => {
                definition.lowered_signature().exact()
            }
        }
    }

    pub fn semantic_signature(&self) -> &ExactCallableSignature {
        match &self.definition {
            SelectedCallableDefinition::Direct(record) => record.signature(),
            SelectedCallableDefinition::Lowered { definition, .. } => {
                definition.semantic_signature().exact()
            }
        }
    }

    pub fn lowering_role(&self) -> crate::MirCallableLoweringRoleV1 {
        match &self.definition {
            SelectedCallableDefinition::Direct(_) => crate::MirCallableLoweringRoleV1::Ordinary,
            SelectedCallableDefinition::Lowered { definition, .. } => *definition.lowering_role(),
        }
    }

    pub fn lowered_gc_effect(&self, source: GcEffect) -> GcEffect {
        match &self.definition {
            SelectedCallableDefinition::Direct(_) => source,
            SelectedCallableDefinition::Lowered { definition, .. } => {
                definition.lowered_signature().gc_effect()
            }
        }
    }
}
