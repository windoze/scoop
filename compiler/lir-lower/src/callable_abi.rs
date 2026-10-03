//! Shared binding of callable ABI publication to actual MIR and LIR bodies.

use scoop_identity::{CallableDefinitionOwner, ExactCallableSignature, PersistentCallableBodyId};
use scoop_lir as lir;
use scoop_mir as mir;

mod error;
pub use error::*;

pub(crate) struct LocalCallableMaterialization<'a> {
    pub(crate) mir: &'a mir::Function,
    pub(crate) lir: &'a lir::Function,
    module: &'a mir::Module,
    signature: &'a ExactCallableSignature,
    target: CallableDefinitionOwner,
}

impl<'a> LocalCallableMaterialization<'a> {
    pub(crate) fn resolve(
        input: &'a mir::ConeMirInput,
        functions: &'a [lir::Function],
        target: impl Into<CallableDefinitionOwner>,
        signature: &ExactCallableSignature,
    ) -> Result<Self, CallableAbiProjectionError> {
        let target = target.into();
        let subject = match target {
            CallableDefinitionOwner::Strong(owner) => {
                mir::CallableSignatureSubject::Strong(owner.callable_owner())
            }
            CallableDefinitionOwner::Odr(member) => mir::CallableSignatureSubject::Odr(member),
        };
        let exact = input
            .foundation()
            .callable_signature(subject)
            .ok_or(CallableAbiProjectionError::MissingMirSignature(target))?;
        if exact.signature() != signature {
            return Err(CallableAbiProjectionError::MirSignature(target));
        }
        let root = input
            .materialization()
            .callable_roots()
            .iter()
            .find(|root| root.subject() == subject)
            .ok_or(CallableAbiProjectionError::MissingMirBody(target))?;
        let body = PersistentCallableBodyId::from_key(&target.body_key())
            .map_err(CallableAbiProjectionError::Identity)?;
        let lir = functions
            .iter()
            .find(|function| function.callable_body.id() == body)
            .ok_or(CallableAbiProjectionError::MissingLirBody(target))?;
        Ok(Self {
            module: input.module(),
            mir: &input.module().functions[root.function()],
            lir,
            signature: exact.signature(),
            target,
        })
    }

    pub(crate) fn canonical_signature(
        &self,
        enums: &lir::EnumDefs,
    ) -> Result<scoop_identity::CanonicalScoopAbiFunctionSignature, CallableAbiProjectionError>
    {
        let root_plan = match self.mir.gc_effect {
            mir::GcEffect::Managed => lir::ExternalCallableRootPlan::ManagedStatepoint,
            mir::GcEffect::NoGc => lir::ExternalCallableRootPlan::NoGc,
        };
        if root_plan.gc_effect() != self.lir.gc_effect {
            return Err(CallableAbiProjectionError::GcEffect(self.target));
        }
        if self.lir.signature.calling_convention() != lir::CallingConvention::Cdecl {
            return Err(CallableAbiProjectionError::CallingConvention(self.target));
        }
        if self.mir.params.len() != self.lir.signature.arguments().len() {
            return Err(CallableAbiProjectionError::ArgumentCount {
                target: self.target,
                mir: self.mir.params.len(),
                lir: self.lir.signature.arguments().len(),
            });
        }
        let abi = crate::native_abi::canonical_scoop_signature(
            self.module,
            enums,
            self.signature.clone(),
            &self
                .mir
                .params
                .iter()
                .map(|parameter| parameter.ty.clone())
                .collect::<Vec<_>>(),
            &self.mir.return_ty,
            self.mir.gc_effect,
            &self.lir.signature,
        );
        Ok(abi)
    }

    pub(crate) fn abi_record(
        &self,
        enums: &lir::EnumDefs,
    ) -> Result<lir::CallableAbiRecordV1, CallableAbiProjectionError> {
        let CallableDefinitionOwner::Strong(target) = self.target else {
            return Err(CallableAbiProjectionError::MirSignature(self.target));
        };
        let abi = self.canonical_signature(enums)?;
        let root_plan = match self.mir.gc_effect {
            mir::GcEffect::Managed => lir::ExternalCallableRootPlan::ManagedStatepoint,
            mir::GcEffect::NoGc => lir::ExternalCallableRootPlan::NoGc,
        };
        lir::CallableAbiRecordV1::new(
            self.module.cone,
            target,
            abi,
            self.lir.signature.calling_convention(),
            root_plan,
        )
        .map_err(|source| CallableAbiProjectionError::Abi {
            target: self.target,
            source,
        })
    }
}
