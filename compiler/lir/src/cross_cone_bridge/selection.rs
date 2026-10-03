use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};

use super::{CrossConeLirBridgeSectionV1, SelectedDependencyLirCallableV1};
pub use scoop_identity::CallableRole;

/// Request-local index into one validated external LIR selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectedExternalLirCallableId(u32);

/// Complete external callable records checked at the dependency boundary.
pub struct SelectedExternalLirSet {
    consumer: ConeIdentity,
    by_declaration:
        BTreeMap<(ConeIdentity, DependencyCallableDeclarationId), SelectedExternalLirCallableId>,
    callables: Vec<SelectedDependencyLirCallableV1>,
}

impl SelectedExternalLirSet {
    #[doc(hidden)]
    pub fn try_from_bridge(
        bridge: &CrossConeLirBridgeSectionV1,
    ) -> Result<Self, SelectedExternalLirSetBuildError> {
        Self::try_from_callables(bridge.artifact(), bridge.selected().to_vec())
    }

    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            consumer,
            by_declaration: BTreeMap::new(),
            callables: Vec::new(),
        }
    }

    pub fn try_from_callables(
        consumer: ConeIdentity,
        mut selected: Vec<SelectedDependencyLirCallableV1>,
    ) -> Result<Self, SelectedExternalLirSetBuildError> {
        selected.sort_unstable_by_key(|callable| {
            (callable.provider(), callable.bridge().declaration())
        });
        if let Some(callable) = selected
            .iter()
            .find(|callable| callable.provider() == consumer)
        {
            return Err(SelectedExternalLirSetBuildError::SelectedCurrentProvider {
                provider: callable.provider(),
            });
        }
        if let Some(pair) = selected.windows(2).find(|pair| {
            (pair[0].provider(), pair[0].bridge().declaration())
                == (pair[1].provider(), pair[1].bridge().declaration())
        }) {
            return Err(SelectedExternalLirSetBuildError::DuplicateCallable {
                provider: pair[0].provider(),
                declaration: pair[0].bridge().declaration(),
            });
        }
        let mut by_declaration = BTreeMap::new();
        let mut callables = Vec::with_capacity(selected.len());
        for selected in selected {
            let index = u32::try_from(callables.len()).map_err(|_| {
                SelectedExternalLirSetBuildError::TooManyCallables {
                    count: callables.len().saturating_add(1),
                }
            })?;
            let id = SelectedExternalLirCallableId(index);
            by_declaration.insert((selected.provider(), selected.bridge().declaration()), id);
            callables.push(selected);
        }
        Ok(Self {
            consumer,
            by_declaration,
            callables,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn callable(
        &self,
        id: SelectedExternalLirCallableId,
    ) -> Option<&SelectedDependencyLirCallableV1> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_for(
        &self,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<SelectedExternalLirCallableId> {
        self.by_declaration.get(&(provider, declaration)).copied()
    }

    pub fn callable_by_target(
        &self,
        provider: ConeIdentity,
        target: scoop_identity::StrongCallableDefinitionOwner,
    ) -> Option<&SelectedDependencyLirCallableV1> {
        self.callables.iter().find(|callable| {
            callable.provider() == provider && callable.bridge().target() == target
        })
    }

    pub fn dependency_callables(&self) -> impl Iterator<Item = &SelectedDependencyLirCallableV1> {
        self.callables.iter()
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedExternalLirSetBuildError {
    TooManyCallables {
        count: usize,
    },
    SelectedCurrentProvider {
        provider: ConeIdentity,
    },
    DuplicateCallable {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
}

impl fmt::Display for SelectedExternalLirSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCallables { count } => write!(
                formatter,
                "external LIR selection contains {count} callables, exceeding the u32 id domain"
            ),
            Self::SelectedCurrentProvider { provider } => write!(
                formatter,
                "external LIR selection names current Cone {provider} as an external provider"
            ),
            Self::DuplicateCallable {
                provider,
                declaration,
            } => write!(
                formatter,
                "external LIR selection contains duplicate callable {provider}:{declaration:?}"
            ),
        }
    }
}

impl std::error::Error for SelectedExternalLirSetBuildError {}
