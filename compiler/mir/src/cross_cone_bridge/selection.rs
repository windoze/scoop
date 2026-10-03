use std::fmt;

use scoop_identity::{ConeIdentity, StrongCallableDefinitionOwner};

use super::{CrossConeMirBridgeSectionV1, SelectedDependencyMirCallableV1};
mod callable;
pub use callable::SelectedExternalMirCallable;
pub use scoop_identity::CallableRole;

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
    objects: Vec<crate::ParamFreeMirObjectValueV1>,
}

impl SelectedExternalMirSet {
    /// Constructs an empty selection for a current-Cone request.
    #[doc(hidden)]
    pub fn empty(consumer: ConeIdentity) -> Self {
        Self {
            consumer,
            callables: Vec::new(),
            objects: Vec::new(),
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
            Vec::new(),
        )
    }

    pub fn try_from_selections(
        consumer: ConeIdentity,
        mut selected: Vec<SelectedExternalMirCallable>,
        mut objects: Vec<crate::ParamFreeMirObjectValueV1>,
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
        objects.sort_unstable_by_key(crate::ParamFreeMirObjectValueV1::value);
        if let Some(object) = objects.iter().find(|object| object.provider() == consumer) {
            return Err(SelectedExternalMirSetBuildError::SelectedCurrentProvider {
                provider: object.provider(),
            });
        }
        if let Some(pair) = objects
            .windows(2)
            .find(|pair| pair[0].value() == pair[1].value())
        {
            return Err(SelectedExternalMirSetBuildError::DuplicateObject(
                pair[0].value(),
            ));
        }
        Ok(Self {
            consumer,
            callables: selected,
            objects,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    /// Complete selection, including compiler-service roles.
    pub fn callables(&self) -> &[SelectedExternalMirCallable] {
        &self.callables
    }

    /// Keeps the selected Scoop entries still used after lowering protocols
    /// such as direct release-time C calls. Retention preserves canonical order.
    pub fn retain_callables(&mut self, keep: impl FnMut(&SelectedExternalMirCallable) -> bool) {
        self.callables.retain(keep);
    }

    pub fn objects(&self) -> &[crate::ParamFreeMirObjectValueV1] {
        &self.objects
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

    pub fn len(&self) -> usize {
        self.callables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedExternalMirSetBuildError {
    DuplicateObject(scoop_identity::PersistentObjectValueId),
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
            Self::DuplicateObject(value) => {
                write!(formatter, "external MIR selection repeats object {value}")
            }
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
