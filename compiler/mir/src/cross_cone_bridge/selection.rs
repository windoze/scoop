use std::fmt;

use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, StrongCallableDefinitionOwner,
};

use super::{CrossConeMirBridgeSectionV1, SelectedDependencyMirCallableV1};
mod callable;
pub use callable::{CallableRole, SelectedExternalMirCallable};

/// The actual provider and typed declaration of one external callable.
/// The MIR arena uses the separate `ExternalCallableUseId` domain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SelectedExternalMirCallableRef {
    provider: ConeIdentity,
    implementation: StrongCallableDefinitionOwner,
}

impl SelectedExternalMirCallableRef {
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub const fn implementation(self) -> StrongCallableDefinitionOwner {
        self.implementation
    }
}

/// Complete canonical external callable selection for one MIR
/// lowering request.
///
/// The input records retain the provider definitions and signatures already
/// checked at the dependency boundary.
pub struct SelectedExternalMirSet {
    consumer: ConeIdentity,
    callables: Vec<SelectedExternalMirCallable>,
}

impl SelectedExternalMirSet {
    /// Constructs an empty selection for a current-Cone request.
    #[doc(hidden)]
    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            consumer,
            callables: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub fn try_from_bridge(
        bridge: &CrossConeMirBridgeSectionV1,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        Self::try_from_callables(bridge.artifact(), bridge.selected().to_vec())
    }

    /// Collects complete typed callable records from the dependency input.
    /// Provider, signature, and implementation checks belong to that boundary.
    #[doc(hidden)]
    pub fn try_from_callables(
        consumer: ConeIdentity,
        selected: Vec<SelectedDependencyMirCallableV1>,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        Self::try_from_selections(
            consumer,
            selected
                .into_iter()
                .map(SelectedExternalMirCallable::dependency)
                .collect(),
        )
    }

    pub fn try_from_selections(
        consumer: ConeIdentity,
        mut selected: Vec<SelectedExternalMirCallable>,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        selected.sort_unstable_by_key(|callable| (callable.provider(), callable.implementation()));
        if let Some(callable) = selected
            .iter()
            .find(|callable| callable.provider() == consumer)
        {
            return Err(SelectedExternalMirSetBuildError::SelectedCurrentProvider {
                provider: callable.provider(),
            });
        }
        if let Some(pair) = selected.windows(2).find(|pair| {
            (pair[0].provider(), pair[0].implementation())
                == (pair[1].provider(), pair[1].implementation())
        }) {
            return Err(SelectedExternalMirSetBuildError::DuplicateCallable {
                provider: pair[0].provider(),
                implementation: pair[0].implementation(),
            });
        }
        Ok(Self {
            consumer,
            callables: selected,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    /// Complete selection, including compiler-service roles.
    pub fn callables(&self) -> &[SelectedExternalMirCallable] {
        &self.callables
    }

    /// Constructs a MIR arena value with the caller's GC effect. LIR checks
    /// the effect against the provider's canonical ABI and root plan.
    pub fn callable_use(
        &self,
        reference: SelectedExternalMirCallableRef,
        gc_effect: crate::GcEffect,
    ) -> Option<crate::ExternalCallableUse> {
        self.resolve_callable(reference).map(|callable| {
            crate::ExternalCallableUse::new(reference, callable.lowered_gc_effect(gc_effect))
        })
    }

    pub fn resolve_callable(
        &self,
        reference: SelectedExternalMirCallableRef,
    ) -> Option<&SelectedExternalMirCallable> {
        self.callables
            .binary_search_by_key(
                &(reference.provider, reference.implementation),
                |callable| (callable.provider(), callable.implementation()),
            )
            .ok()
            .map(|index| &self.callables[index])
    }

    pub fn callable_for(
        &self,
        provider: ConeIdentity,
        implementation: StrongCallableDefinitionOwner,
    ) -> Option<SelectedExternalMirCallableRef> {
        let reference = SelectedExternalMirCallableRef {
            provider,
            implementation,
        };
        self.resolve_callable(reference).map(|_| reference)
    }

    /// Projection into the existing ordinary-callable metadata partition.
    pub fn dependency_callables(&self) -> impl Iterator<Item = &SelectedDependencyMirCallableV1> {
        self.callables
            .iter()
            .filter_map(SelectedExternalMirCallable::direct_record)
    }

    pub fn initialization_cycle(&self) -> Option<SelectedExternalMirCallableRef> {
        self.callables
            .iter()
            .find(|callable| callable.role() == CallableRole::InitializationCycle)
            .map(|callable| SelectedExternalMirCallableRef {
                provider: callable.provider(),
                implementation: callable.implementation(),
            })
    }

    /// Adds the service before any MIR uses are allocated and seals one complete set.
    pub fn with_initialization_cycle(
        self,
        record: SelectedDependencyMirCallableV1,
    ) -> Result<Self, SelectedExternalMirSetBuildError> {
        if self.initialization_cycle().is_some() {
            return Err(SelectedExternalMirSetBuildError::DuplicateInitializationCycle);
        }
        if record.signature().receiver().is_present()
            || !matches!(
                record.declaration(),
                DependencyCallableDeclarationId::Function(_)
            )
        {
            return Err(SelectedExternalMirSetBuildError::InvalidInitializationCycle);
        }
        let mut callables = self.callables;
        callables.push(SelectedExternalMirCallable::initialization_cycle(record));
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
pub enum SelectedExternalMirSetBuildError {
    DuplicateInitializationCycle,
    InvalidInitializationCycle,
    SelectedCurrentProvider {
        provider: ConeIdentity,
    },
    DuplicateCallable {
        provider: ConeIdentity,
        implementation: StrongCallableDefinitionOwner,
    },
}

impl fmt::Display for SelectedExternalMirSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateInitializationCycle => {
                formatter.write_str("MIR selection contains duplicate initialization services")
            }
            Self::InvalidInitializationCycle => formatter
                .write_str("MIR initialization service must name a function without a receiver"),
            Self::SelectedCurrentProvider { provider } => write!(
                formatter,
                "external MIR selection names current Cone {provider} as an external provider"
            ),
            Self::DuplicateCallable {
                provider,
                implementation,
            } => write!(
                formatter,
                "external MIR selection contains duplicate callable {provider}:{implementation:?}"
            ),
        }
    }
}

impl std::error::Error for SelectedExternalMirSetBuildError {}

#[cfg(test)]
mod tests;
