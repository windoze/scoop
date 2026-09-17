//! Typed external callables selected from ordinary dependency Cones.

use std::fmt;

use scoop_identity::{
    CallableBodyKey, CanonicalScoopAbiFunctionSignature, ConeIdentity,
    DependencyCallableDeclarationId, GcEffect as CanonicalGcEffect, PersistentCallableBodyId,
    PersistentSymbolRequest, StrongCallableDefinitionOwner,
};
use scoop_wire::HashError;

use crate::{
    CallingConvention, DependencyExternalCallableRootPlanV1, GcEffect, ScoopAbiSignature,
    SelectedDependencyLirCallableV1,
    external_callable_abi::{CanonicalScoopAbiMismatch, validate_canonical_scoop_abi},
};

/// One ordinary dependency definition declared by the consumer's LLVM
/// module. The selected semantic bridge owns all link-facing facts; the
/// target-specific signature is admitted only after exact ABI replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyExternalCallable {
    selected: SelectedDependencyLirCallableV1,
    body: PersistentCallableBodyId,
    signature: ScoopAbiSignature,
}

impl DependencyExternalCallable {
    pub(crate) fn new(
        selected: SelectedDependencyLirCallableV1,
        signature: ScoopAbiSignature,
    ) -> Result<Self, DependencyExternalBuildError> {
        let bridge = selected.bridge();
        validate_canonical_scoop_abi(bridge.abi_signature(), &signature).map_err(|error| {
            match error {
                CanonicalScoopAbiMismatch::ArgumentCount { expected, actual } => {
                    DependencyExternalBuildError::AbiArgumentCount { expected, actual }
                }
                CanonicalScoopAbiMismatch::Argument { index } => {
                    DependencyExternalBuildError::AbiArgumentMismatch { index }
                }
                CanonicalScoopAbiMismatch::Result => {
                    DependencyExternalBuildError::AbiResultMismatch
                }
            }
        })?;
        if signature.calling_convention() != bridge.calling_convention() {
            return Err(DependencyExternalBuildError::CallingConventionMismatch {
                expected: bridge.calling_convention(),
                actual: signature.calling_convention(),
            });
        }
        let expected_effect = match bridge.abi_signature().gc_effect() {
            CanonicalGcEffect::Managed => GcEffect::Managed,
            CanonicalGcEffect::NoGc => GcEffect::NoGc,
        };
        if root_plan_gc_effect(bridge.root_plan()) != expected_effect {
            return Err(DependencyExternalBuildError::RootProtocolMismatch);
        }
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(bridge.target()))
            .map_err(DependencyExternalBuildError::Identity)?;
        Ok(Self {
            selected,
            body,
            signature,
        })
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.selected.provider()
    }

    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.selected.bridge().declaration()
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.selected.bridge().target()
    }

    pub const fn body(&self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn canonical_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        self.selected.bridge().abi_signature()
    }

    pub const fn signature(&self) -> &ScoopAbiSignature {
        &self.signature
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.selected.bridge().calling_convention()
    }

    pub const fn root_plan(&self) -> DependencyExternalCallableRootPlanV1 {
        self.selected.bridge().root_plan()
    }

    pub const fn gc_effect(&self) -> GcEffect {
        root_plan_gc_effect(self.root_plan())
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.selected.bridge().expected_symbol()
    }

    pub const fn required_definition(&self) -> scoop_identity::ObjectDefinitionPlanId {
        self.selected.bridge().required_definition()
    }
}

const fn root_plan_gc_effect(root_plan: DependencyExternalCallableRootPlanV1) -> GcEffect {
    match root_plan {
        DependencyExternalCallableRootPlanV1::ManagedStatepoint => GcEffect::Managed,
        DependencyExternalCallableRootPlanV1::NoGc => GcEffect::NoGc,
    }
}

#[derive(Debug)]
pub enum DependencyExternalBuildError {
    AbiArgumentCount {
        expected: usize,
        actual: usize,
    },
    AbiArgumentMismatch {
        index: usize,
    },
    AbiResultMismatch,
    CallingConventionMismatch {
        expected: CallingConvention,
        actual: CallingConvention,
    },
    RootProtocolMismatch,
    Identity(HashError),
}

impl fmt::Display for DependencyExternalBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiArgumentCount { expected, actual } => write!(
                formatter,
                "dependency external signature requires {expected} logical ABI arguments, found {actual}"
            ),
            Self::AbiArgumentMismatch { index } => write!(
                formatter,
                "dependency external canonical and physical ABI disagree at argument {index}"
            ),
            Self::AbiResultMismatch => formatter
                .write_str("dependency external canonical and physical result ABI disagree"),
            Self::CallingConventionMismatch { expected, actual } => write!(
                formatter,
                "dependency external calling convention {actual:?} disagrees with {expected:?}"
            ),
            Self::RootProtocolMismatch => formatter.write_str(
                "dependency external caller root protocol disagrees with its canonical GC effect",
            ),
            Self::Identity(source) => {
                write!(
                    formatter,
                    "cannot derive dependency external identity: {source}"
                )
            }
        }
    }
}

impl std::error::Error for DependencyExternalBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(source) => Some(source),
            Self::AbiArgumentCount { .. }
            | Self::AbiArgumentMismatch { .. }
            | Self::AbiResultMismatch
            | Self::CallingConventionMismatch { .. }
            | Self::RootProtocolMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests;
