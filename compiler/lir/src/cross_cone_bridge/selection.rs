use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};

use super::{CrossConeLirBridgeSectionV1, SelectedDependencyLirCallableV1};

/// Request-local index into one validated ordinary-dependency LIR selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SelectedDependencyLirCallableId(u32);

/// Complete canonical ordinary-dependency callable authority for one LIR
/// lowering request.
///
/// Construction consumes a locally validated bridge. Closure validation is
/// responsible for proving every selected record against its terminal
/// provider before this set reaches lowering.
pub struct SelectedDependencyLirSet {
    consumer: ConeIdentity,
    by_declaration:
        BTreeMap<(ConeIdentity, DependencyCallableDeclarationId), SelectedDependencyLirCallableId>,
    callables: Vec<SelectedDependencyLirCallableV1>,
}

impl SelectedDependencyLirSet {
    #[doc(hidden)]
    pub fn try_from_bridge(
        bridge: &CrossConeLirBridgeSectionV1,
    ) -> Result<Self, SelectedDependencyLirSetBuildError> {
        Self::try_from_callables(bridge.artifact(), bridge.selected().to_vec())
    }

    /// Seals a producer-side selection after its records have been projected
    /// from an already validated dependency closure.
    #[doc(hidden)]
    pub fn try_from_callables(
        consumer: ConeIdentity,
        mut selected: Vec<SelectedDependencyLirCallableV1>,
    ) -> Result<Self, SelectedDependencyLirSetBuildError> {
        selected.sort_unstable_by_key(|callable| {
            (callable.provider(), callable.bridge().declaration())
        });
        if let Some(callable) = selected
            .iter()
            .find(|callable| callable.provider() == consumer)
        {
            return Err(
                SelectedDependencyLirSetBuildError::SelectedCurrentProvider {
                    provider: callable.provider(),
                },
            );
        }
        if selected
            .iter()
            .any(|callable| callable.provider() == ConeIdentity::CORE)
        {
            return Err(SelectedDependencyLirSetBuildError::SelectedTrustedCore);
        }
        if let Some(pair) = selected.windows(2).find(|pair| {
            (pair[0].provider(), pair[0].bridge().declaration())
                == (pair[1].provider(), pair[1].bridge().declaration())
        }) {
            return Err(SelectedDependencyLirSetBuildError::DuplicateCallable {
                provider: pair[0].provider(),
                declaration: pair[0].bridge().declaration(),
            });
        }
        let mut by_declaration = BTreeMap::new();
        let mut callables = Vec::with_capacity(selected.len());
        for selected in selected {
            let index = u32::try_from(callables.len()).map_err(|_| {
                SelectedDependencyLirSetBuildError::TooManyCallables {
                    count: callables.len().saturating_add(1),
                }
            })?;
            let id = SelectedDependencyLirCallableId(index);
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
        id: SelectedDependencyLirCallableId,
    ) -> Option<&SelectedDependencyLirCallableV1> {
        self.callables.get(id.0 as usize)
    }

    pub fn callable_for(
        &self,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    ) -> Option<SelectedDependencyLirCallableId> {
        self.by_declaration.get(&(provider, declaration)).copied()
    }

    pub fn callables(&self) -> &[SelectedDependencyLirCallableV1] {
        &self.callables
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedDependencyLirSetBuildError {
    TooManyCallables {
        count: usize,
    },
    SelectedCurrentProvider {
        provider: ConeIdentity,
    },
    SelectedTrustedCore,
    DuplicateCallable {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
}

impl fmt::Display for SelectedDependencyLirSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCallables { count } => write!(
                formatter,
                "dependency LIR selection contains {count} callables, exceeding the u32 id domain"
            ),
            Self::SelectedCurrentProvider { provider } => write!(
                formatter,
                "dependency LIR selection names current Cone {provider} as an ordinary provider"
            ),
            Self::SelectedTrustedCore => formatter
                .write_str("dependency LIR selection names trusted core as an ordinary provider"),
            Self::DuplicateCallable {
                provider,
                declaration,
            } => write!(
                formatter,
                "dependency LIR selection contains duplicate callable {provider}:{declaration:?}"
            ),
        }
    }
}

impl std::error::Error for SelectedDependencyLirSetBuildError {}
