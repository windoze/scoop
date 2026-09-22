//! Intrinsic contracts attached to the shared callable declaration surface.

use scoop_hir::{
    CallableImplementationV1, CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    IntrinsicCallableContractError, IntrinsicFunctionKind,
};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, IdentityReferenceError, SourceDeclarationKey,
    ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

pub(crate) fn validate_intrinsic_declarations<'a>(
    interface: &CrossConeHirInterfaceSectionV1,
    identities: &ValidatedIdentityGraph,
    providers: impl IntoIterator<Item = (ConeIdentity, &'a CoreBootstrapInterfaceSectionV1)> + Clone,
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
        let provider = source.origin();
        let mut selected = None;
        for (identity, section) in providers.clone() {
            meter
                .charge_work(1, &path)
                .map_err(CrossConeIntrinsicDeclarationError::Resource)?;
            if identity == provider && selected.replace(section).is_some() {
                return Err(CrossConeIntrinsicDeclarationError::DuplicateProvider(
                    provider,
                ));
            }
        }
        let roles = selected
            .and_then(CoreBootstrapInterfaceSectionV1::compiler_protocol_definitions)
            .ok_or(CrossConeIntrinsicDeclarationError::MissingTypeRoles(
                provider,
            ))?
            .compiler_protocols();
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
    MissingTypeRoles(ConeIdentity),
    DuplicateProvider(ConeIdentity),
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
            Self::MissingTypeRoles(_)
            | Self::DuplicateProvider(_)
            | Self::DuplicateKind(_)
            | Self::CallableKind(_) => None,
        }
    }
}
