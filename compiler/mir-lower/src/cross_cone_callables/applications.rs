//! Callable bindings for concrete nominal applications already emitted by MIR.

use super::*;
use scoop_identity::{CallableDefinitionOwner, CallableMaterialization};

pub(super) fn append(
    local: &hir::LocalConcreteHir,
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    records: &mut Vec<mir::ParamFreeMirCallableBindingV1>,
) -> Result<(), Error> {
    let mut producer = Producer {
        input,
        authority: mir::MirCallableBridgeAuthority {
            identities,
            foundation: input.foundation(),
            types,
        },
        records,
    };
    for (id, function) in local.functions.iter() {
        if !matches!(
            function.materialization.context(),
            CallableMaterializationContext::Application(_)
        ) || (function.receiver.method().is_none() && !function.is_suspend)
            || matches!(function.kind, hir::concrete::FunctionKind::Intrinsic(intrinsic)
                if intrinsic.kind.equality_member().is_none())
        {
            continue;
        }
        let role = match function.materialization.template() {
            CallableTemplateOwner::Function(id) => roles::project(
                local,
                function,
                Declaration::Function(id),
                modality(function),
            )?,
            CallableTemplateOwner::Accessor(id) => roles::project(
                local,
                function,
                Declaration::PropertyAccessor(id),
                modality(function),
            )?,
            CallableTemplateOwner::GenericFunction(_) => mir::MirCallableLoweringRoleV1::Ordinary,
            _ => continue,
        };
        let signature = crate::source_callables::exact_function_signature(local, id);
        producer.record(
            function.materialization,
            signature.clone(),
            signature,
            crate::lowering_support::lower_gc_effect(function.attributes.gc_effect),
            role,
        )?;
    }
    for (id, constructor) in local.class_constructors.iter() {
        if !matches!(
            constructor.materialization.context(),
            CallableMaterializationContext::Application(_)
        ) {
            continue;
        }
        let lowered = crate::source_callables::exact_class_initializer_signature(local, id);
        let owner = lowered
            .receiver()
            .into_option()
            .ok_or(Error::ApplicationMaterialization(
                constructor.materialization,
            ))?;
        let semantic = ExactCallableSignature::new(
            lowered.effect(),
            None,
            lowered.parameters().to_vec(),
            owner,
        );
        producer.record(
            constructor.materialization,
            semantic,
            lowered,
            mir::GcEffect::Managed,
            mir::MirCallableLoweringRoleV1::ClassInitializer { owner },
        )?;
    }
    for (id, constructor) in local.struct_constructors.iter() {
        if !matches!(
            constructor.materialization.context(),
            CallableMaterializationContext::Application(_)
        ) {
            continue;
        }
        let signature = crate::source_callables::exact_struct_constructor_signature(local, id);
        let owner = signature.result();
        let (source_gc, role) = match constructor.kind {
            hir::concrete::StructConstructorKind::Primary => (
                mir::GcEffect::Managed,
                mir::MirCallableLoweringRoleV1::PrimaryValueConstructor { owner },
            ),
            hir::concrete::StructConstructorKind::Secondary { gc_effect, .. } => (
                crate::lowering_support::lower_gc_effect(gc_effect),
                mir::MirCallableLoweringRoleV1::ValueConstructor { owner },
            ),
        };
        producer.record(
            constructor.materialization,
            signature.clone(),
            signature,
            source_gc,
            role,
        )?;
    }
    Ok(())
}

fn modality(function: &hir::concrete::Function) -> hir::CallableModalityV1 {
    if function
        .receiver
        .method()
        .is_some_and(|method| method.modifier == hir::MethodModifier::Abstract)
    {
        hir::CallableModalityV1::Abstract
    } else {
        hir::CallableModalityV1::Final
    }
}

struct Producer<'a> {
    input: &'a mir::ConeMirInput,
    authority: mir::MirCallableBridgeAuthority<'a>,
    records: &'a mut Vec<mir::ParamFreeMirCallableBindingV1>,
}

impl Producer<'_> {
    fn record(
        &mut self,
        materialization: CallableMaterialization,
        semantic: ExactCallableSignature,
        lowered: ExactCallableSignature,
        source_gc: mir::GcEffect,
        role: mir::MirCallableLoweringRoleV1,
    ) -> Result<(), Error> {
        let source = self
            .input
            .module()
            .meta
            .source_callable_materializations
            .get_by_materialization(materialization)
            .ok_or(Error::ApplicationMaterialization(materialization))?;
        let mir::CallableSignatureSubject::Odr(member) = source.signature_record().subject() else {
            return Err(Error::ApplicationMaterialization(materialization));
        };
        if source.signature_record().signature() != &lowered {
            return Err(Error::ApplicationMaterialization(materialization));
        }
        let lowered = self
            .input
            .module()
            .meta
            .callable_signatures
            .get(source.signature_record().subject())
            .ok_or(Error::ApplicationMaterialization(materialization))?
            .signature()
            .clone();
        let origin = match materialization.template() {
            CallableTemplateOwner::Generated(callable) => {
                let key = self
                    .authority
                    .identities
                    .canonical_key::<_, scoop_identity::GeneratedCallableKey>(callable)
                    .map_err(mir::MirCallableBridgeError::from)?;
                mir::MirCallableOriginV1::Generated {
                    callable,
                    role: key.as_ref().clone(),
                }
            }
            _ => {
                let CallableMaterializationContext::Application(application) =
                    materialization.context()
                else {
                    return Err(Error::ApplicationMaterialization(materialization));
                };
                mir::MirCallableOriginV1::Application(application)
            }
        };
        reserve(self.records, 1)?;
        self.records
            .push(mir::ParamFreeMirCallableBindingV1::try_new(
                self.authority,
                origin,
                CallableDefinitionOwner::Odr(member),
                mir::MirBridgeCallableSignatureV1::new(semantic, source_gc),
                mir::MirBridgeCallableSignatureV1::new(
                    lowered,
                    self.input.module().functions[source.function()].gc_effect,
                ),
                role,
            )?);
        Ok(())
    }
}
