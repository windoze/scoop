//! Nominal signature classification shared by dependency callable bridges.

use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, DependencyCallableDeclarationId, Effect, ExactCallableSignature,
    GcEffect, PersistentExactTypeId, PersistentTypeId, SignatureTypeKey,
};

use crate::{CallableImplementationV1, CallableInterfaceRecordV1, PublicDeclarationOwnerV1};

mod leaves;

/// Resolves exact nominal signatures from the actual declaration scope.
/// Machine availability, access and layout are checked by their own stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalExactLeafClassifierV1 {
    leaves: Vec<(PersistentTypeId, PersistentExactTypeId)>,
}

impl NominalExactLeafClassifierV1 {
    /// Returns the exact identity of a concrete nominal in the supplied
    /// public surface, or the language builtins Unit and Any. ABI and runtime shape
    /// requirements are validated by the later MIR/LIR bridge checks.
    pub fn classify(&self, signature: &SignatureTypeKey) -> Option<PersistentExactTypeId> {
        let SignatureTypeKey::Nominal(source) = signature else {
            return None;
        };
        self.leaves
            .binary_search_by_key(source, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.leaves[index].1)
    }

    /// Resolves the signature of an ordinary param-free top-level callable or
    /// extension. This does not establish its implementation or ABI.
    pub fn classify_callable(
        &self,
        callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ParamFreeNominalCallableV1>, NominalCallableClassificationError> {
        let Some(declaration) = eligible_declaration(callable) else {
            return Ok(None);
        };
        let Some(signature) = self.exact_signature(callable)? else {
            return Ok(None);
        };
        Ok(Some(ParamFreeNominalCallableV1 {
            declaration,
            signature,
            gc_effect: callable.effects().gc_effect(),
        }))
    }

    fn exact_signature(
        &self,
        callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ExactCallableSignature>, NominalCallableClassificationError> {
        let receiver = match callable.receiver() {
            Some(receiver) => match self.classify(receiver) {
                Some(exact) => Some(exact),
                None => return Ok(None),
            },
            None => None,
        };
        let parameter_count = callable.parameters().parameters().len();
        let mut parameters = Vec::new();
        parameters.try_reserve_exact(parameter_count).map_err(|_| {
            NominalCallableClassificationError::Allocation {
                requested_slots: parameter_count,
            }
        })?;
        for parameter in callable.parameters().parameters() {
            let Some(exact) = self.classify(parameter.value_type()) else {
                return Ok(None);
            };
            parameters.push(exact);
        }
        let Some(result) = self.classify(callable.result()) else {
            return Ok(None);
        };
        Ok(Some(ExactCallableSignature::new(
            callable.effects().execution(),
            receiver,
            parameters,
            result,
        )))
    }
}

fn eligible_declaration(
    callable: &CallableInterfaceRecordV1,
) -> Option<DependencyCallableDeclarationId> {
    if !matches!(
        callable.owner(),
        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension
    ) || !callable.type_parameters().is_empty()
        || callable.effects().execution() != Effect::Ordinary
        || callable.effects().implementation() != CallableImplementationV1::Scoop
    {
        return None;
    }
    match callable.declaration() {
        CallableTemplateOrigin::Function(declaration) => {
            Some(DependencyCallableDeclarationId::Function(declaration))
        }
        CallableTemplateOrigin::Accessor(declaration) => Some(
            DependencyCallableDeclarationId::PropertyAccessor(declaration),
        ),
        CallableTemplateOrigin::GenericFunction(_)
        | CallableTemplateOrigin::Constructor(_)
        | CallableTemplateOrigin::VariantConstructor(_) => None,
    }
}

/// Resolved nominal signature of a param-free dependency callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeNominalCallableV1 {
    declaration: DependencyCallableDeclarationId,
    signature: ExactCallableSignature,
    gc_effect: GcEffect,
}

impl ParamFreeNominalCallableV1 {
    pub const fn declaration(&self) -> DependencyCallableDeclarationId {
        self.declaration
    }

    pub const fn implementation(&self) -> scoop_identity::StrongCallableDefinitionOwner {
        self.declaration.implementation()
    }

    pub const fn signature(&self) -> &ExactCallableSignature {
        &self.signature
    }

    pub const fn gc_effect(&self) -> GcEffect {
        self.gc_effect
    }

    pub const fn is_no_gc(&self) -> bool {
        matches!(self.gc_effect, GcEffect::NoGc)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalExactLeafClassifierBuildError {
    Resource(scoop_wire::WireError),
    Identity(scoop_wire::HashError),
}

impl fmt::Display for NominalExactLeafClassifierBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to build the nominal exact-leaf classifier: {self:?}"
        )
    }
}

impl std::error::Error for NominalExactLeafClassifierBuildError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalCallableClassificationError {
    Allocation { requested_slots: usize },
    Resource(scoop_wire::WireError),
}

impl fmt::Display for NominalCallableClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to classify a nominal dependency callable: {self:?}"
        )
    }
}

impl std::error::Error for NominalCallableClassificationError {}

#[cfg(test)]
mod tests;
