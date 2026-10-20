mod declarations;

use declarations::{declare_global, define_global, digest_value, require_callable_entry};

use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::StructType;
use inkwell::values::{FunctionValue, GlobalValue, StructValue};
use scoop_lir::{
    ConeIdentity, ObjectDefinitionAtomId, PersistentInitializationUnitId, PersistentSymbolRequest,
    StrongInitializationCallableRefPlanV1, StrongInitializationRegistrationSchedulePlanV1,
    StrongInitializationUnitRegistrationPlan, StrongInitializationUnitRegistrationPlanSet,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;
use crate::target::ValidatedBackendProfile;

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const INITIALIZATION_UNIT_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4e49;
const INITIALIZATION_UNIT_DESCRIPTOR_SIZE: u64 = 288;

#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongInitializationUnitRegistrationV1<'ctx> {
    unit: PersistentInitializationUnitId,
    cell: GlobalValue<'ctx>,
    diagnostic_atom: ObjectDefinitionAtomId,
    diagnostic: GlobalValue<'ctx>,
    registration_descriptor: GlobalValue<'ctx>,
}

impl<'ctx> EmittedStrongInitializationUnitRegistrationV1<'ctx> {
    pub const fn unit(self) -> PersistentInitializationUnitId {
        self.unit
    }

    pub const fn cell(self) -> GlobalValue<'ctx> {
        self.cell
    }

    pub const fn diagnostic_atom(self) -> ObjectDefinitionAtomId {
        self.diagnostic_atom
    }

    pub const fn diagnostic(self) -> GlobalValue<'ctx> {
        self.diagnostic
    }

    pub const fn registration_descriptor(self) -> GlobalValue<'ctx> {
        self.registration_descriptor
    }
}

#[derive(Clone, Debug)]
pub struct EmittedStrongInitializationUnitRegistrationSetV1<'ctx> {
    producer: ConeIdentity,
    registrations: Vec<EmittedStrongInitializationUnitRegistrationV1<'ctx>>,
}

impl<'ctx> EmittedStrongInitializationUnitRegistrationSetV1<'ctx> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[EmittedStrongInitializationUnitRegistrationV1<'ctx>] {
        &self.registrations
    }
}

/// Emit the `ic` cell and its single runtime-registration `nr`
/// definitions for every initialization unit.
pub(crate) fn emit_strong_initialization_unit_registrations_v1<'ctx, D: Clone>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    profile: ValidatedBackendProfile,
    plan: &StrongInitializationUnitRegistrationPlanSet<D>,
) -> Result<EmittedStrongInitializationUnitRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let registrations = plan
        .registrations()
        .iter()
        .map(|registration| emit_registration(context, llvm, profile, &types, registration))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmittedStrongInitializationUnitRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

fn emit_registration<'ctx, D>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    profile: ValidatedBackendProfile,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: &StrongInitializationUnitRegistrationPlan<D>,
) -> Result<EmittedStrongInitializationUnitRegistrationV1<'ctx>, CodegenError> {
    let initializer = require_callable_entry(llvm, plan.initializer())?;
    let ensure = require_callable_entry(llvm, plan.ensure())?;
    let semantic = plan.semantic();
    let cell = define_global(
        llvm,
        plan.cell_symbol(),
        types.initialization_cell,
        llvm.get_global(plan.cell_symbol().symbol().as_str()),
        false,
        types.initialization_cell.const_zero().into(),
    );
    // The mutable coordinator cell is also a primary definition atom. Keep
    // its zero initializer file-backed for the definition fingerprint.
    crate::metadata_sections::set_section(cell, profile.writable_storage_section());
    let storage_registration = declare_global(
        llvm,
        plan.storage().registration_symbol(),
        types.static_storage_descriptor,
        llvm.get_global(plan.storage().registration_symbol().symbol().as_str()),
    );
    let failure_registration = declare_global(
        llvm,
        plan.failure_root().registration_symbol(),
        types.static_storage_descriptor,
        llvm.get_global(plan.failure_root().registration_symbol().symbol().as_str()),
    );
    let diagnostic_initializer = context.const_string(semantic.diagnostic_path().as_bytes(), true);
    let diagnostic = llvm.add_global(
        diagnostic_initializer.get_type(),
        None,
        &format!("{}.diagnostic", plan.registration_symbol().symbol()),
    );
    diagnostic.set_linkage(Linkage::Private);
    diagnostic.set_constant(true);
    diagnostic.set_initializer(&diagnostic_initializer);
    crate::metadata_sections::set_section(diagnostic, profile.c_string_section());

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let zero_digest = types.digest.const_zero();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(INITIALIZATION_UNIT_DESCRIPTOR_MAGIC, false)
            .into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(INITIALIZATION_UNIT_DESCRIPTOR_SIZE, false)
            .into(),
    ]);
    let identity = super::registration_identity::registration_identity_value(
        context,
        types,
        semantic.unit().as_array(),
        plan.definition_owner(),
    );
    let diagnostic_span = types.byte_span.const_named_struct(&[
        diagnostic.as_pointer_value().into(),
        i64.const_int(semantic.diagnostic_path().len() as u64, false)
            .into(),
    ]);
    let (gateway_id, gateway_entry) = match plan.schedule() {
        StrongInitializationRegistrationSchedulePlanV1::EagerStartup { gateway, .. } => (
            digest_value(context, types.digest, gateway.body().as_array()),
            require_callable_entry(llvm, **gateway)?
                .as_global_value()
                .as_pointer_value(),
        ),
        StrongInitializationRegistrationSchedulePlanV1::LazyAccess => (
            zero_digest,
            context
                .ptr_type(inkwell::AddressSpace::default())
                .const_null(),
        ),
    };
    let registration_value = types.initialization_unit_descriptor.const_named_struct(&[
        prefix.into(),
        identity.into(),
        i32.const_int(u64::from(semantic.schedule().tag()), false)
            .into(),
        i32.const_zero().into(),
        diagnostic_span.into(),
        cell.as_pointer_value().into(),
        storage_registration.as_pointer_value().into(),
        failure_registration.as_pointer_value().into(),
        digest_value(context, types.digest, semantic.initializer().as_array()).into(),
        digest_value(context, types.digest, semantic.ensure().as_array()).into(),
        initializer.as_global_value().as_pointer_value().into(),
        ensure.as_global_value().as_pointer_value().into(),
        gateway_id.into(),
        gateway_entry.into(),
    ]);
    let registration = define_global(
        llvm,
        plan.registration_symbol(),
        types.initialization_unit_descriptor,
        llvm.get_global(plan.registration_symbol().symbol().as_str()),
        true,
        registration_value.into(),
    );

    Ok(EmittedStrongInitializationUnitRegistrationV1 {
        unit: semantic.unit(),
        cell,
        diagnostic_atom: plan.diagnostic_atom(),
        diagnostic,
        registration_descriptor: registration,
    })
}

#[cfg(test)]
mod tests;
