//! Classification of the M23-5 executable dependency-callable subset.

use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, CoreBuiltinNominal, DependencyCallableDeclarationId, Effect,
    ExactCallableSignature, ExactTypeKey, GcEffect, PersistentExactTypeId, PersistentTypeId,
    SignatureTypeKey,
};

use crate::{
    CallableImplementationV1, CallableInterfaceRecordV1, NominalInterfaceRecordV1,
    PublicDeclarationOwnerV1, SourceNominalId,
};

/// Maps the shared nominal declaration surface to exact leaves accepted by
/// the current core-closed cross-Cone callable bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreClosedExactLeafClassifierV1 {
    leaves: Vec<(PersistentTypeId, PersistentExactTypeId)>,
}

impl CoreClosedExactLeafClassifierV1 {
    pub fn try_from_nominal_interfaces(
        nominals: &[NominalInterfaceRecordV1],
    ) -> Result<Self, CoreClosedExactLeafClassifierBuildError> {
        let target_count = nominals.len();
        let mut leaves = Vec::new();
        leaves.try_reserve_exact(target_count + 1).map_err(|_| {
            CoreClosedExactLeafClassifierBuildError::Allocation {
                requested_slots: target_count + 1,
            }
        })?;
        let unit = CoreBuiltinNominal::Unit.identity_record().id();
        let unit_exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(unit))
            .map_err(CoreClosedExactLeafClassifierBuildError::Identity)?;
        leaves.push((unit, unit_exact));
        for nominal in nominals {
            let SourceNominalId::Concrete(source) = nominal.declaration() else {
                continue;
            };
            let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source))
                .map_err(CoreClosedExactLeafClassifierBuildError::Identity)?;
            leaves.push((source, exact));
        }
        leaves.sort_unstable_by_key(|(source, _)| *source);
        leaves.dedup_by_key(|(source, _)| *source);
        Ok(Self { leaves })
    }

    /// Returns the exact identity of a concrete nominal in the supplied
    /// public surface, or the language builtin Unit. ABI and runtime shape
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

    #[cfg(test)]
    pub(crate) fn from_exact_leaves_for_test(
        mut leaves: Vec<(PersistentTypeId, PersistentExactTypeId)>,
    ) -> Self {
        leaves.sort_unstable_by_key(|(source, _)| *source);
        leaves.dedup_by_key(|(source, _)| *source);
        Self { leaves }
    }

    /// Refines one public callable interface into the complete executable
    /// M23-5 bridge shape, or reports that it remains semantic-only.
    pub fn classify_callable(
        &self,
        callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ParamFreeCoreClosedCallableV1>, CoreClosedCallableClassificationError> {
        let Some(declaration) = eligible_declaration(callable) else {
            return Ok(None);
        };
        let Some(signature) = self.exact_signature(callable)? else {
            return Ok(None);
        };
        Ok(Some(ParamFreeCoreClosedCallableV1 {
            declaration,
            signature,
            gc_effect: callable.effects().gc_effect(),
        }))
    }

    fn exact_signature(
        &self,
        callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ExactCallableSignature>, CoreClosedCallableClassificationError> {
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
            CoreClosedCallableClassificationError::Allocation {
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

/// Complete HIR proof needed to emit one M23-5 dependency callable use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeCoreClosedCallableV1 {
    declaration: DependencyCallableDeclarationId,
    signature: ExactCallableSignature,
    gc_effect: GcEffect,
}

impl ParamFreeCoreClosedCallableV1 {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreClosedExactLeafClassifierBuildError {
    Allocation { requested_slots: usize },
    Identity(scoop_wire::HashError),
}

impl fmt::Display for CoreClosedExactLeafClassifierBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to build the nominal exact-leaf classifier: {self:?}"
        )
    }
}

impl std::error::Error for CoreClosedExactLeafClassifierBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreClosedCallableClassificationError {
    Allocation { requested_slots: usize },
}

impl fmt::Display for CoreClosedCallableClassificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to classify a core-closed dependency callable: {self:?}"
        )
    }
}

impl std::error::Error for CoreClosedCallableClassificationError {}

#[cfg(test)]
mod tests;
