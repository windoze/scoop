//! Nominal signature classification shared by dependency callable bridges.

use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, DependencyCallableDeclarationId, ExactCallableSignature, GcEffect,
    PersistentExactTypeId, PersistentTypeId, SignatureTypeKey, StrongCallableDefinitionOwner,
};

use crate::{
    CallableDeclarationRecordV1, CallableImplementationV1, CallableModalityV1,
    PublicDeclarationOwnerV1, SourceNominalId,
};

mod leaves;
mod signatures;

/// Resolves exact nominal signatures from the actual declaration scope.
/// Machine availability, access and layout are checked by their own stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalExactLeafClassifierV1 {
    leaves: Vec<(PersistentTypeId, PersistentExactTypeId)>,
    generic_sources: Vec<scoop_identity::PersistentGenericTypeId>,
}

impl NominalExactLeafClassifierV1 {
    /// Resolves a param-free callable's source signature and actual implementation.
    /// Constructors have no receiver in their source signature; their physical
    /// lowering signature is supplied by the defining provider.
    pub fn classify_callable(
        &self,
        callable: &CallableDeclarationRecordV1,
    ) -> Result<Option<ParamFreeNominalCallableV1>, NominalCallableClassificationError> {
        let Some(implementation) = eligible_declaration(callable) else {
            return Ok(None);
        };
        let Some(signature) = self.exact_signature(callable)? else {
            return Ok(None);
        };
        Ok(Some(ParamFreeNominalCallableV1 {
            implementation,
            signature,
            gc_effect: callable.effects().provider_entry_gc_effect(),
            modality: callable.modality(),
        }))
    }

    fn exact_signature(
        &self,
        callable: &CallableDeclarationRecordV1,
    ) -> Result<Option<ExactCallableSignature>, NominalCallableClassificationError> {
        let nominal_receiver = match (callable.declaration(), callable.owner()) {
            (CallableTemplateOrigin::Constructor(_), _) => None,
            (_, PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner))) => {
                Some(SignatureTypeKey::Nominal(owner))
            }
            _ => None,
        };
        let receiver = match nominal_receiver.as_ref().or_else(|| callable.receiver()) {
            Some(receiver) => match self.classify(receiver)? {
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
            let Some(exact) = self.classify(parameter.value_type())? else {
                return Ok(None);
            };
            parameters.push(exact);
        }
        let Some(result) = self.classify(callable.result())? else {
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
    callable: &CallableDeclarationRecordV1,
) -> Option<StrongCallableDefinitionOwner> {
    let direct_owner = match callable.owner() {
        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => true,
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(_)) => true,
        PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(_)) => false,
    };
    if !direct_owner
        || !callable.type_parameters().is_empty()
        || matches!(
            callable.effects().implementation(),
            CallableImplementationV1::Intrinsic(_)
        )
    {
        return None;
    }
    match callable.declaration() {
        CallableTemplateOrigin::Function(declaration) => {
            Some(StrongCallableDefinitionOwner::Function(declaration))
        }
        CallableTemplateOrigin::Accessor(declaration) => {
            Some(StrongCallableDefinitionOwner::PropertyAccessor(declaration))
        }
        CallableTemplateOrigin::Constructor(declaration) => {
            Some(StrongCallableDefinitionOwner::Constructor(declaration))
        }
        CallableTemplateOrigin::GenericFunction(_)
        | CallableTemplateOrigin::VariantConstructor(_) => None,
    }
}

/// Resolved nominal signature of a param-free dependency callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamFreeNominalCallableV1 {
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
    gc_effect: GcEffect,
    modality: CallableModalityV1,
}

impl ParamFreeNominalCallableV1 {
    /// The existing direct-callable table stores functions and accessors.
    /// Other definitions retain their complete M23-6 lowering records.
    pub const fn direct_declaration(&self) -> Option<DependencyCallableDeclarationId> {
        if matches!(self.modality, CallableModalityV1::Abstract)
            || matches!(self.signature.effect(), scoop_identity::Effect::Suspend)
        {
            return None;
        }
        match self.implementation {
            StrongCallableDefinitionOwner::Function(id) => {
                Some(DependencyCallableDeclarationId::Function(id))
            }
            StrongCallableDefinitionOwner::PropertyAccessor(id) => {
                Some(DependencyCallableDeclarationId::PropertyAccessor(id))
            }
            StrongCallableDefinitionOwner::Constructor(_)
            | StrongCallableDefinitionOwner::GeneratedCallable(_) => None,
        }
    }

    pub const fn implementation(&self) -> scoop_identity::StrongCallableDefinitionOwner {
        self.implementation
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
    Identity(scoop_wire::HashError),
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
