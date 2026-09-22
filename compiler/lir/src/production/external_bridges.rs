//! Compiler-service references use the shared dependency callable and descriptor records.

use crate::{ExternalTypeDescriptor, Module, SelectedDependencyLirCallableV1};
use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, StrongCallableDefinitionOwner,
};

mod errors;
mod references;
mod runtime_string;
pub use errors::*;
mod wire;
pub use wire::DecodedStrongExternalLirBridgeSurfaceV1;
#[cfg(test)]
use wire::DecodedStrongExternalLirBridgeV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongExternalLirBridgeV1 {
    Callable(Box<SelectedDependencyLirCallableV1>),
    TypeDescriptor(ExternalTypeDescriptor),
}

impl StrongExternalLirBridgeV1 {
    pub const fn provider(&self) -> ConeIdentity {
        match self {
            Self::Callable(callable) => callable.provider(),
            Self::TypeDescriptor(descriptor) => descriptor.provider(),
        }
    }

    pub fn expected_symbol(&self) -> scoop_identity::PersistentSymbolRequest {
        match self {
            Self::Callable(callable) => callable.bridge().expected_symbol(),
            Self::TypeDescriptor(descriptor) => descriptor.expected_symbol(),
        }
    }

    fn sort_key(&self) -> (u8, [u8; 32]) {
        match self {
            Self::Callable(callable) => {
                (1, *callable.bridge().expected_symbol().key().owner_bytes())
            }
            Self::TypeDescriptor(descriptor) => (2, *descriptor.target().as_array()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongExternalLirBridgeSurfaceV1 {
    producer: ConeIdentity,
    bridges: Vec<StrongExternalLirBridgeV1>,
}

impl StrongExternalLirBridgeSurfaceV1 {
    pub const fn empty(producer: ConeIdentity) -> Self {
        Self {
            producer,
            bridges: Vec::new(),
        }
    }

    pub fn from_module(module: &Module) -> Result<Self, StrongExternalLirBridgeBuildError> {
        let mut bridges = Vec::with_capacity(module.meta.external_callables.len() + 1);
        for (_, callable) in module.meta.external_callables.iter() {
            if callable.origin() != crate::ExternalCallableOrigin::InitializationCycle {
                continue;
            }
            let StrongCallableDefinitionOwner::Function(function) = callable.target() else {
                return Err(
                    StrongExternalLirBridgeBuildError::InvalidInitializationTarget(
                        callable.target(),
                    ),
                );
            };
            bridges.push(StrongExternalLirBridgeV1::Callable(Box::new(
                SelectedDependencyLirCallableV1::new(
                    callable.provider(),
                    DependencyCallableDeclarationId::Function(function),
                    callable.target(),
                    callable.canonical_signature().clone(),
                    callable.calling_convention(),
                    callable.root_plan(),
                )
                .map_err(StrongExternalLirBridgeBuildError::Callable)?,
            )));
        }
        if let Some(descriptor) = Self::runtime_string(module)? {
            bridges.push(StrongExternalLirBridgeV1::TypeDescriptor(descriptor));
        }
        Self::try_new(module.cone, bridges)
    }

    pub fn try_new(
        producer: ConeIdentity,
        mut bridges: Vec<StrongExternalLirBridgeV1>,
    ) -> Result<Self, StrongExternalLirBridgeBuildError> {
        for bridge in &bridges {
            if bridge.provider() == producer {
                return Err(StrongExternalLirBridgeBuildError::SelfImport { provider: producer });
            }
            match bridge {
                StrongExternalLirBridgeV1::Callable(callable) => {
                    callable
                        .bridge()
                        .callable_abi()
                        .link_contract(callable.provider())
                        .map_err(StrongExternalLirBridgeBuildError::CallableContract)?;
                }
                StrongExternalLirBridgeV1::TypeDescriptor(descriptor) => {
                    Self::validate_runtime_string(*descriptor)?;
                }
            }
        }
        bridges.sort_unstable_by_key(StrongExternalLirBridgeV1::sort_key);
        if let Some(pair) = bridges
            .windows(2)
            .find(|pair| pair[0].sort_key() == pair[1].sort_key())
        {
            return Err(StrongExternalLirBridgeBuildError::DuplicateTarget(
                pair[0].sort_key(),
            ));
        }
        Ok(Self { producer, bridges })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn bridges(&self) -> &[StrongExternalLirBridgeV1] {
        &self.bridges
    }
}

#[cfg(test)]
mod tests;
