use scoop_identity::{CallableTemplateOrigin, SourceDeclarationKey, SourceDeclarationKind};

use super::*;
use crate::{CallableDeclarationRecordV1, CallableImplementationV1};

impl CoreCompilerProtocolSurfaceV1 {
    /// Replays a published intrinsic against its actual source declaration and typed roles.
    pub fn validate_intrinsic_declaration(
        &self,
        source: &SourceDeclarationKey,
        callable: &CallableDeclarationRecordV1,
    ) -> Result<(), IntrinsicCallableContractError> {
        let CallableImplementationV1::Intrinsic(kind) = callable.effects().implementation() else {
            return Err(IntrinsicCallableContractError::ExpectedIntrinsic);
        };
        if !matches!(
            callable.declaration(),
            CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::GenericFunction(_)
        ) || source.declaration_kind() != SourceDeclarationKind::Function
        {
            return Err(IntrinsicCallableContractError::CallableKind(kind));
        }
        if matches!(
            kind,
            IntrinsicFunctionKind::Atomic(_) | IntrinsicFunctionKind::MaybeUninit(_)
        ) {
            // Atomic and MaybeUninit owners are ordinary nominal declarations,
            // not members of the fixed bootstrap protocol surface.
            return Ok(());
        }
        if let Some((_, role)) = self
            .fixed_intrinsic_callables()
            .into_iter()
            .find(|(role_kind, _)| *role_kind == kind)
        {
            let matches = match (role.definition(), callable.declaration()) {
                (
                    crate::CoreProtocolCallableDefinitionV1::Function(expected),
                    CallableTemplateOrigin::Function(actual),
                ) => expected == actual,
                (
                    crate::CoreProtocolCallableDefinitionV1::GenericFunction(expected),
                    CallableTemplateOrigin::GenericFunction(actual),
                ) => expected == actual,
                _ => false,
            };
            if !matches {
                return Err(IntrinsicCallableContractError::FixedRole(kind));
            }
        }
        let expected_owner = expected_operation_owner(self, kind);
        let owners = source.owners().owners();
        if !match expected_owner {
            Some(expected) => owners == [expected],
            None => owners.is_empty(),
        } {
            return Err(IntrinsicCallableContractError::Owner(kind));
        }
        let own_count = operation_own_type_parameter_count(kind);
        if source.duplicate_signature().type_parameter_count() != own_count
            || callable.type_parameters().len_u32() != own_count
        {
            return Err(IntrinsicCallableContractError::TypeParameters(kind));
        }
        let expected = expected_operation_signature(self, kind);
        if callable.effects().execution() != expected.effect()
            || callable.receiver().is_some()
            || callable.parameters().parameters().len() != expected.parameters().len()
            || !callable
                .parameters()
                .parameters()
                .iter()
                .zip(expected.parameters())
                .all(|(actual, expected)| actual.value_type() == expected)
            || callable.result() != expected.result()
        {
            return Err(IntrinsicCallableContractError::Signature(kind));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntrinsicCallableContractError {
    ExpectedIntrinsic,
    CallableKind(IntrinsicFunctionKind),
    Owner(IntrinsicFunctionKind),
    FixedRole(IntrinsicFunctionKind),
    TypeParameters(IntrinsicFunctionKind),
    Signature(IntrinsicFunctionKind),
}

impl std::fmt::Display for IntrinsicCallableContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid intrinsic callable contract: {self:?}")
    }
}

impl std::error::Error for IntrinsicCallableContractError {}
