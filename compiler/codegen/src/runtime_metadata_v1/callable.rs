use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::values::GlobalValue;
use scoop_lir::{
    ConeIdentity, ObjectDefinitionAtomId, PersistentCallableBodyId,
    StrongCallableRegistrationPlanSetV1, StrongCallableRegistrationPlanV1,
};

use super::{RuntimeMetadataV1Types, registration_identity_value};
use crate::CodegenError;

mod context_keys;

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5043_414c;
const CALLABLE_REGISTRATION_DESCRIPTOR_SIZE: u64 = 144;

/// One emitted runtime callable registration.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongCallableRegistrationV1<'ctx> {
    primary_atom: ObjectDefinitionAtomId,
    body: PersistentCallableBodyId,
    descriptor: GlobalValue<'ctx>,
}

impl<'ctx> EmittedStrongCallableRegistrationV1<'ctx> {
    pub const fn primary_atom(self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }
}

/// Canonically ordered emission result for every callable in one Cone.
#[derive(Clone, Debug)]
pub struct EmittedStrongCallableRegistrationSetV1<'ctx> {
    producer: ConeIdentity,
    registrations: Vec<EmittedStrongCallableRegistrationV1<'ctx>>,
    pub(super) context_atoms: Vec<crate::atom_boundaries::GlobalAtomMaterializationV1<'ctx>>,
}

impl<'ctx> EmittedStrongCallableRegistrationSetV1<'ctx> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[EmittedStrongCallableRegistrationV1<'ctx>] {
        &self.registrations
    }
}

/// Emit every strong callable registration from the closed LIR production
/// plan.
pub(crate) fn emit_strong_callable_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongCallableRegistrationPlanSetV1,
    surface: &scoop_lir::ObjectSymbolSurfaceV1,
    profile: crate::target::ValidatedBackendProfile,
    body: PersistentCallableBodyId,
) -> Result<EmittedStrongCallableRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let mut context_atoms = Vec::new();
    let registrations = plan
        .registrations()
        .iter()
        .filter(|registration| registration.body() == body)
        .map(|registration| {
            let runtime = plan
                .runtime_scans()
                .callable(registration.body())
                .ok_or_else(|| {
                    CodegenError(format!(
                        "missing callable support plan {}",
                        registration.body()
                    ))
                })?;
            let keys = context_keys::emit(
                context,
                llvm,
                surface,
                profile,
                *registration,
                runtime,
                &mut context_atoms,
            )?;
            emit_registration(
                context,
                llvm,
                &types,
                *registration,
                keys,
                runtime.context_keys().len() as u64,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmittedStrongCallableRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
        context_atoms,
    })
}

fn emit_registration<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: StrongCallableRegistrationPlanV1,
    keys: inkwell::values::PointerValue<'ctx>,
    key_count: u64,
) -> Result<EmittedStrongCallableRegistrationV1<'ctx>, CodegenError> {
    let descriptor_request = plan.symbol();
    let descriptor_symbol = descriptor_request.symbol();
    let entry_request = plan.entry_symbol();
    let entry_symbol = entry_request.symbol();
    let entry = llvm.get_function(entry_symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "callable entry `{entry_symbol}` is not declared in the LLVM module"
        ))
    })?;
    let descriptor = if let Some(global) = llvm.get_global(descriptor_symbol.as_str()) {
        global
    } else {
        let global = llvm.add_global(
            types.callable_registration_descriptor,
            None,
            descriptor_symbol.as_str(),
        );
        global.set_linkage(Linkage::External);
        global
    };

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC, false)
            .into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(CALLABLE_REGISTRATION_DESCRIPTOR_SIZE, false)
            .into(),
    ]);
    let identity = registration_identity_value(
        context,
        types,
        plan.body().as_array(),
        plan.definition_owner(),
    );
    let value = types.callable_registration_descriptor.const_named_struct(&[
        prefix.into(),
        identity.into(),
        entry.as_global_value().as_pointer_value().into(),
        keys.into(),
        i64.const_int(key_count, false).into(),
    ]);
    descriptor.set_constant(true);
    descriptor.set_initializer(&value);
    crate::emission::apply_persistent_linkage(&descriptor, descriptor_request, true)?;

    Ok(EmittedStrongCallableRegistrationV1 {
        primary_atom: plan.primary_atom(),
        body: plan.body(),
        descriptor,
    })
}

#[cfg(test)]
mod tests;
