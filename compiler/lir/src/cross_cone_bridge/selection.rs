use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};

use super::{CrossConeLirBridgeSectionV1, SelectedDependencyLirCallableV1};
mod callable;
pub use callable::{CallableRole, SelectedExternalLirCallable};

/// Request-local index into one validated external LIR selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectedExternalLirCallableId(u32);

/// Complete canonical external callable authority for one LIR
/// lowering request.
///
/// Construction consumes a locally validated bridge. Closure validation is
/// responsible for proving every selected record against its terminal
/// provider before this set reaches lowering.
pub struct SelectedExternalLirSet {
    consumer: ConeIdentity,
    by_declaration:
        BTreeMap<(ConeIdentity, DependencyCallableDeclarationId), SelectedExternalLirCallableId>,
    callables: Vec<SelectedExternalLirCallable>,
}

impl SelectedExternalLirSet {
    #[doc(hidden)]
    pub fn try_from_bridge(
        bridge: &CrossConeLirBridgeSectionV1,
    ) -> Result<Self, SelectedExternalLirSetBuildError> {
        Self::try_from_callables(bridge.artifact(), bridge.selected().to_vec())
    }

    /// Seals a producer-side selection after its records have been projected
    /// from an already validated dependency closure.
    #[doc(hidden)]
    pub fn try_from_callables(
        consumer: ConeIdentity,
        selected: Vec<SelectedDependencyLirCallableV1>,
    ) -> Result<Self, SelectedExternalLirSetBuildError> {
        Self::try_from_selections(
            consumer,
            selected
                .into_iter()
                .map(SelectedExternalLirCallable::dependency)
                .collect(),
        )
    }

    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            consumer,
            by_declaration: BTreeMap::new(),
            callables: Vec::new(),
        }
    }

    /// Seals all projected roles together before lowering allocates any uses.
    pub fn try_from_role_records(
        consumer: ConeIdentity,
        records: Vec<(CallableRole, SelectedDependencyLirCallableV1)>,
    ) -> Result<Self, SelectedExternalLirSetBuildError> {
        Self::try_from_selections(
            consumer,
            records
                .into_iter()
                .map(|(role, record)| match role {
                    CallableRole::Ordinary => SelectedExternalLirCallable::dependency(record),
                    CallableRole::InitializationCycle => {
                        SelectedExternalLirCallable::initialization_cycle(record)
                    }
                })
                .collect(),
        )
    }

    fn try_from_selections(
        consumer: ConeIdentity,
        mut selected: Vec<SelectedExternalLirCallable>,
    ) -> Result<Self, SelectedExternalLirSetBuildError> {
        let mut has_initialization_cycle = false;
        for callable in &selected {
            if callable.role() == CallableRole::InitializationCycle {
                if has_initialization_cycle {
                    return Err(SelectedExternalLirSetBuildError::DuplicateInitializationCycle);
                }
                if !matches!(
                    callable.bridge().declaration(),
                    DependencyCallableDeclarationId::Function(_)
                ) || callable
                    .bridge()
                    .abi_signature()
                    .signature()
                    .receiver()
                    .is_present()
                {
                    return Err(SelectedExternalLirSetBuildError::InvalidInitializationCycle);
                }
                has_initialization_cycle = true;
            }
        }
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
    ) -> Option<&SelectedExternalLirCallable> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_for(
        &self,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<SelectedExternalLirCallableId> {
        self.by_declaration.get(&(provider, declaration)).copied()
    }

    pub fn dependency_callables(&self) -> impl Iterator<Item = &SelectedDependencyLirCallableV1> {
        self.callables
            .iter()
            .filter(|callable| callable.role() == CallableRole::Ordinary)
            .map(SelectedExternalLirCallable::record)
    }

    pub fn initialization_cycle(&self) -> Option<SelectedExternalLirCallableId> {
        self.callables
            .iter()
            .position(|callable| callable.role() == CallableRole::InitializationCycle)
            .map(|index| SelectedExternalLirCallableId(index as u32))
    }

    pub fn with_initialization_cycle(
        self,
        record: SelectedDependencyLirCallableV1,
    ) -> Result<Self, SelectedExternalLirSetBuildError> {
        let mut callables = self.callables;
        callables.push(SelectedExternalLirCallable::initialization_cycle(record));
        Self::try_from_selections(self.consumer, callables)
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
    DuplicateInitializationCycle,
    InvalidInitializationCycle,
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
            Self::DuplicateInitializationCycle => {
                formatter.write_str("LIR selection contains duplicate initialization services")
            }
            Self::InvalidInitializationCycle => formatter
                .write_str("LIR initialization service must name a function without a receiver"),
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
