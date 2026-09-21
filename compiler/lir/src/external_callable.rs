//! Typed external Scoop callables shared by compiler protocols and dependency selections.

use std::fmt;

use scoop_identity::{
    CallableBodyKey, CanonicalScoopAbiFunctionSignature, ConeIdentity,
    DependencyCallableDeclarationId, GcEffect as CanonicalGcEffect, ObjectDefinitionPlanId,
    PersistentCallableBodyId, PersistentSymbolRequest, StrongCallableDefinitionOwner,
};
use scoop_wire::HashError;

use crate::{
    CallingConvention, GcEffect, ScoopAbiSignature, SelectedDependencyLirCallableV1,
    external_callable_abi::{CanonicalScoopAbiMismatch, validate_canonical_scoop_abi},
};

/// One external definition declared by the consumer's LLVM module. The
/// selected semantic bridge owns all link-facing facts; the target-specific
/// signature is admitted only after exact ABI replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalCallable {
    origin: ExternalCallableOrigin,
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
    body: PersistentCallableBodyId,
    canonical_signature: CanonicalScoopAbiFunctionSignature,
    signature: ScoopAbiSignature,
    calling_convention: CallingConvention,
    root_plan: crate::ExternalCallableRootPlan,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalCallableOrigin {
    Legacy(DependencyCallableDeclarationId),
    LayoutV1,
    InitializationCycle,
}

impl ExternalCallable {
    pub(crate) fn new(
        selected: SelectedDependencyLirCallableV1,
        signature: ScoopAbiSignature,
    ) -> Result<Self, ExternalCallableBuildError> {
        let bridge = selected.bridge();
        validate_signature(
            bridge.abi_signature(),
            &signature,
            ExternalCallableRootPlan::from_dependency(bridge.root_plan()),
        )?;
        if signature.calling_convention() != bridge.calling_convention() {
            return Err(ExternalCallableBuildError::CallingConventionMismatch {
                expected: bridge.calling_convention(),
                actual: signature.calling_convention(),
            });
        }
        let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(bridge.target()))
            .map_err(ExternalCallableBuildError::Identity)?;
        Ok(Self {
            origin: ExternalCallableOrigin::Legacy(bridge.declaration()),
            provider: selected.provider(),
            target: bridge.target(),
            body,
            canonical_signature: bridge.abi_signature().clone(),
            signature,
            calling_convention: bridge.calling_convention(),
            root_plan: crate::ExternalCallableRootPlan::from_dependency(bridge.root_plan()),
            expected_symbol: bridge.expected_symbol(),
            required_definition: bridge.required_definition(),
        })
    }

    pub(crate) fn from_layout_v1(
        provider: ConeIdentity,
        record: &crate::ExactCallableAbiExportV1,
        expected_symbol: PersistentSymbolRequest,
        required_definition: ObjectDefinitionPlanId,
        signature: ScoopAbiSignature,
        enums: &crate::EnumDefs,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<Self, crate::LayoutExternalMaterializationError> {
        record
            .validate_physical_signature(
                enums,
                &signature,
                protocol_effect(record.call_protocol()),
                meter,
            )
            .map_err(crate::LayoutExternalMaterializationError::CallableAbi)?;
        let physical = record.physical_definition();
        if physical.provider() != provider
            || physical.symbol() != expected_symbol
            || physical.definition() != required_definition
        {
            return Err(crate::LayoutExternalMaterializationError::PhysicalCallable(
                record.target(),
            ));
        }
        Ok(Self {
            origin: ExternalCallableOrigin::LayoutV1,
            provider,
            target: record.target(),
            body: record.definition().semantic_id(),
            canonical_signature: record.canonical_signature().clone(),
            signature,
            calling_convention: record.calling_convention(),
            root_plan: match record.call_protocol() {
                crate::ExactCallableProtocolV1::OrdinaryManaged => {
                    crate::ExternalCallableRootPlan::ManagedStatepoint
                }
                crate::ExactCallableProtocolV1::OrdinaryNoGc => {
                    crate::ExternalCallableRootPlan::NoGc
                }
            },
            expected_symbol,
            required_definition,
        })
    }

    pub const fn origin(&self) -> ExternalCallableOrigin {
        self.origin
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn legacy_declaration(&self) -> Option<DependencyCallableDeclarationId> {
        match self.origin {
            ExternalCallableOrigin::Legacy(declaration) => Some(declaration),
            ExternalCallableOrigin::LayoutV1 | ExternalCallableOrigin::InitializationCycle => None,
        }
    }

    pub const fn target(&self) -> StrongCallableDefinitionOwner {
        self.target
    }

    pub const fn body(&self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn canonical_signature(&self) -> &CanonicalScoopAbiFunctionSignature {
        &self.canonical_signature
    }

    pub const fn signature(&self) -> &ScoopAbiSignature {
        &self.signature
    }

    pub const fn calling_convention(&self) -> CallingConvention {
        self.calling_convention
    }

    pub const fn root_plan(&self) -> crate::ExternalCallableRootPlan {
        self.root_plan
    }

    pub const fn gc_effect(&self) -> GcEffect {
        self.root_plan().gc_effect()
    }

    pub const fn expected_symbol(&self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(&self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

fn validate_signature(
    canonical: &CanonicalScoopAbiFunctionSignature,
    physical: &ScoopAbiSignature,
    root_plan: ExternalCallableRootPlan,
) -> Result<(), ExternalCallableBuildError> {
    validate_canonical_scoop_abi(canonical, physical).map_err(|error| match error {
        CanonicalScoopAbiMismatch::ArgumentCount { expected, actual } => {
            ExternalCallableBuildError::AbiArgumentCount { expected, actual }
        }
        CanonicalScoopAbiMismatch::Argument { index } => {
            ExternalCallableBuildError::AbiArgumentMismatch { index }
        }
        CanonicalScoopAbiMismatch::Result => ExternalCallableBuildError::AbiResultMismatch,
    })?;
    let expected = match canonical.gc_effect() {
        CanonicalGcEffect::Managed => GcEffect::Managed,
        CanonicalGcEffect::NoGc => GcEffect::NoGc,
    };
    if root_plan.gc_effect() != expected {
        return Err(ExternalCallableBuildError::RootProtocolMismatch);
    }
    Ok(())
}

fn protocol_effect(protocol: crate::ExactCallableProtocolV1) -> GcEffect {
    match protocol {
        crate::ExactCallableProtocolV1::OrdinaryManaged => GcEffect::Managed,
        crate::ExactCallableProtocolV1::OrdinaryNoGc => GcEffect::NoGc,
    }
}

#[derive(Debug)]
pub enum ExternalCallableBuildError {
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
    ProtocolContract(crate::CoreExternalBuildError),
}

impl fmt::Display for ExternalCallableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbiArgumentCount { expected, actual } => write!(
                formatter,
                "external signature requires {expected} logical ABI arguments, found {actual}"
            ),
            Self::AbiArgumentMismatch { index } => write!(
                formatter,
                "external canonical and physical ABI disagree at argument {index}"
            ),
            Self::AbiResultMismatch => {
                formatter.write_str("external canonical and physical result ABI disagree")
            }
            Self::CallingConventionMismatch { expected, actual } => write!(
                formatter,
                "external calling convention {actual:?} disagrees with {expected:?}"
            ),
            Self::RootProtocolMismatch => formatter
                .write_str("external caller root protocol disagrees with its canonical GC effect"),
            Self::ProtocolContract(source) => source.fmt(formatter),
            Self::Identity(source) => {
                write!(formatter, "cannot derive external identity: {source}")
            }
        }
    }
}

impl std::error::Error for ExternalCallableBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(source) => Some(source),
            Self::ProtocolContract(source) => Some(source),
            Self::AbiArgumentCount { .. }
            | Self::AbiArgumentMismatch { .. }
            | Self::AbiResultMismatch
            | Self::CallingConventionMismatch { .. }
            | Self::RootProtocolMismatch => None,
        }
    }
}

mod protocol;
mod roots;
pub use roots::ExternalCallableRootPlan;

#[cfg(test)]
mod tests;
