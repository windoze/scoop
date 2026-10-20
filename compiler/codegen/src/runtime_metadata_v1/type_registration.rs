use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::values::GlobalValue;
use scoop_lir::{
    ConeIdentity, ObjectDefinitionAtomId, PersistentExactTypeId, StrongTypeRegistrationPlan,
    StrongTypeRegistrationPlanSet,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const TYPE_REGISTRATION_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5054_5950;
const TYPE_REGISTRATION_DESCRIPTOR_SIZE: u64 = 144;

/// One emitted runtime type registration.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongTypeRegistrationV1<'ctx> {
    primary_atom: ObjectDefinitionAtomId,
    exact_type: PersistentExactTypeId,
    descriptor: GlobalValue<'ctx>,
}

impl<'ctx> EmittedStrongTypeRegistrationV1<'ctx> {
    pub const fn primary_atom(self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }
}

/// Canonically ordered emission result for every local exact type in one Cone.
#[derive(Clone, Debug)]
pub struct EmittedStrongTypeRegistrationSetV1<'ctx> {
    producer: ConeIdentity,
    registrations: Vec<EmittedStrongTypeRegistrationV1<'ctx>>,
}

impl<'ctx> EmittedStrongTypeRegistrationSetV1<'ctx> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[EmittedStrongTypeRegistrationV1<'ctx>] {
        &self.registrations
    }
}

/// Emit every strong type registration from the closed LIR production plan.
pub(crate) fn emit_strong_type_registrations_v1<
    'ctx,
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongTypeRegistrationPlanSet<D, C>,
) -> Result<EmittedStrongTypeRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let registrations = plan
        .registrations()
        .iter()
        .map(|registration| emit_registration(context, llvm, &types, registration))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmittedStrongTypeRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

fn emit_registration<'ctx, D: Copy, C>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: &StrongTypeRegistrationPlan<D, C>,
) -> Result<EmittedStrongTypeRegistrationV1<'ctx>, CodegenError> {
    let descriptor_symbol = plan.descriptor_symbol().symbol();
    let type_descriptor = llvm.get_global(descriptor_symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "type descriptor `{descriptor_symbol}` is not declared in the LLVM module"
        ))
    })?;
    let descriptor = llvm
        .get_global(plan.symbol().symbol().as_str())
        .unwrap_or_else(|| {
            let global = llvm.add_global(
                types.type_registration_descriptor,
                None,
                plan.symbol().symbol().as_str(),
            );
            global.set_linkage(Linkage::External);
            global
        });

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(TYPE_REGISTRATION_DESCRIPTOR_MAGIC, false)
            .into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(TYPE_REGISTRATION_DESCRIPTOR_SIZE, false)
            .into(),
    ]);
    let identity = super::registration_identity::registration_identity_value(
        context,
        types,
        plan.exact_type().as_array(),
        plan.definition_owner(),
    );
    let value = types.type_registration_descriptor.const_named_struct(&[
        prefix.into(),
        identity.into(),
        i64.const_int(plan.runtime_type().get(), false).into(),
        i64.const_zero().into(),
        type_descriptor.as_pointer_value().into(),
    ]);
    descriptor.set_constant(true);
    descriptor.set_initializer(&value);
    descriptor.set_linkage(match plan.definition_owner() {
        scoop_lir::RegistrationDefinitionOwner::Strong => Linkage::External,
        scoop_lir::RegistrationDefinitionOwner::Odr { .. } => Linkage::WeakODR,
    });

    Ok(EmittedStrongTypeRegistrationV1 {
        primary_atom: plan.primary_atom(),
        exact_type: plan.exact_type(),
        descriptor,
    })
}

#[cfg(test)]
mod tests;
