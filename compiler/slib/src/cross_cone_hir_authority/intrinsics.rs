//! Intrinsic contracts attached to the shared callable declaration surface.

use scoop_hir::{
    CallableImplementationV1, CoreBootstrapInterfaceSectionV1, CoreHirInterfaceBranchV1,
    CrossConeHirInterfaceSectionV1, IntrinsicCallableContractError, IntrinsicFunctionKind,
};
use scoop_identity::{
    CallableTemplateOrigin, IdentityReferenceError, SourceDeclarationKey, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

pub(crate) fn validate_intrinsic_declarations<'a>(
    interface: &CrossConeHirInterfaceSectionV1,
    identities: &ValidatedIdentityGraph,
    providers: impl IntoIterator<Item = &'a CoreBootstrapInterfaceSectionV1>,
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeIntrinsicDeclarationError> {
    let path = WirePath::root();
    let callables = interface.callable_interfaces().records();
    meter
        .charge_work(callables.len() as u64, &path)
        .map_err(CrossConeIntrinsicDeclarationError::Resource)?;
    let mut intrinsics = Vec::new();
    for callable in callables {
        let CallableImplementationV1::Intrinsic(kind) = callable.effects().implementation() else {
            continue;
        };
        meter
            .charge_work(1 + intrinsics.len() as u64, &path)
            .map_err(CrossConeIntrinsicDeclarationError::Resource)?;
        if intrinsics.iter().any(|(seen, _)| *seen == kind) {
            return Err(CrossConeIntrinsicDeclarationError::DuplicateKind(kind));
        }
        meter
            .try_reserve_collection_slots(&mut intrinsics, 1, &path)
            .map_err(CrossConeIntrinsicDeclarationError::Resource)?;
        intrinsics.push((kind, callable));
    }
    if intrinsics.is_empty() {
        return Ok(());
    }
    let mut roles = None;
    for provider in providers {
        meter
            .charge_work(1, &path)
            .map_err(CrossConeIntrinsicDeclarationError::Resource)?;
        if let CoreHirInterfaceBranchV1::Core(interface) = provider.core_interface() {
            if roles.replace(interface.compiler_protocols()).is_some() {
                return Err(CrossConeIntrinsicDeclarationError::ConflictingTypeRoles);
            }
        }
    }
    let roles = roles.ok_or(CrossConeIntrinsicDeclarationError::MissingTypeRoles)?;
    for (_, callable) in intrinsics {
        meter
            .charge_work(1, &path)
            .map_err(CrossConeIntrinsicDeclarationError::Resource)?;
        let source = match callable.declaration() {
            CallableTemplateOrigin::Function(id) => {
                identities.canonical_key::<_, SourceDeclarationKey>(id)
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                identities.canonical_key::<_, SourceDeclarationKey>(id)
            }
            declaration => {
                return Err(CrossConeIntrinsicDeclarationError::CallableKind(
                    declaration,
                ));
            }
        }
        .map_err(CrossConeIntrinsicDeclarationError::Identity)?;
        roles
            .validate_intrinsic_declaration(&source, callable)
            .map_err(|source| CrossConeIntrinsicDeclarationError::Contract {
                declaration: callable.declaration(),
                source,
            })?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum CrossConeIntrinsicDeclarationError {
    MissingTypeRoles,
    ConflictingTypeRoles,
    DuplicateKind(IntrinsicFunctionKind),
    CallableKind(CallableTemplateOrigin),
    Identity(IdentityReferenceError),
    Resource(WireError),
    Contract {
        declaration: CallableTemplateOrigin,
        source: IntrinsicCallableContractError,
    },
}

impl std::fmt::Display for CrossConeIntrinsicDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid shared intrinsic declaration: {self:?}")
    }
}

impl std::error::Error for CrossConeIntrinsicDeclarationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Contract { source, .. } => Some(source),
            Self::MissingTypeRoles
            | Self::ConflictingTypeRoles
            | Self::DuplicateKind(_)
            | Self::CallableKind(_) => None,
        }
    }
}
