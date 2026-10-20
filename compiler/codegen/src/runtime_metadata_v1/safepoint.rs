use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::values::{GlobalValue, StructValue};
use scoop_lir::{
    ConeIdentity, ObjectDefinitionAtomId, PersistentSafepointSiteId,
    StrongSafepointRegistrationPlanSetV1, StrongSafepointRegistrationPlanV1,
};

use super::{RuntimeMetadataV1Types, registration_identity_value};
use crate::CodegenError;

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5053_5054;
const SAFEPOINT_REGISTRATION_DESCRIPTOR_SIZE: u64 = 168;

/// One emitted runtime safepoint registration.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongSafepointRegistrationV1<'ctx> {
    primary_atom: ObjectDefinitionAtomId,
    site: PersistentSafepointSiteId,
    descriptor: GlobalValue<'ctx>,
}

impl<'ctx> EmittedStrongSafepointRegistrationV1<'ctx> {
    pub const fn primary_atom(self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }
}

/// Canonically ordered emission result for every safepoint in one Cone.
#[derive(Clone, Debug)]
pub struct EmittedStrongSafepointRegistrationSetV1<'ctx> {
    producer: ConeIdentity,
    registrations: Vec<EmittedStrongSafepointRegistrationV1<'ctx>>,
}

impl<'ctx> EmittedStrongSafepointRegistrationSetV1<'ctx> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[EmittedStrongSafepointRegistrationV1<'ctx>] {
        &self.registrations
    }
}

/// Emit all strong safepoint registrations from the closed LIR production
/// plan.
pub(crate) fn emit_strong_safepoint_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongSafepointRegistrationPlanSetV1,
    body: scoop_lir::PersistentCallableBodyId,
    root_count: impl Fn(scoop_lir::SafepointId) -> Option<usize>,
) -> Result<EmittedStrongSafepointRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let registrations = plan
        .registrations()
        .iter()
        .filter(|registration| registration.owner() == body)
        .filter_map(|registration| {
            root_count(registration.safepoint()).map(|count| (*registration, count))
        })
        .map(|(registration, count)| {
            let count = u32::try_from(count)
                .map_err(|_| CodegenError("physical root count exceeds u32::MAX".into()))?;
            emit_registration(context, llvm, &types, registration, count)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmittedStrongSafepointRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

fn emit_registration<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: StrongSafepointRegistrationPlanV1,
    root_count: u32,
) -> Result<EmittedStrongSafepointRegistrationV1<'ctx>, CodegenError> {
    let request = plan.symbol();
    let symbol = request.symbol();
    let descriptor = if let Some(global) = llvm.get_global(symbol.as_str()) {
        global
    } else {
        let global = llvm.add_global(
            types.safepoint_registration_descriptor,
            None,
            symbol.as_str(),
        );
        global.set_linkage(Linkage::External);
        global
    };

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC, false)
            .into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(SAFEPOINT_REGISTRATION_DESCRIPTOR_SIZE, false)
            .into(),
    ]);
    let identity = registration_identity_value(
        context,
        types,
        plan.site().as_array(),
        plan.definition_owner(),
    );
    let value = types
        .safepoint_registration_descriptor
        .const_named_struct(&[
            prefix.into(),
            identity.into(),
            i64.const_int(plan.safepoint().get(), false).into(),
            i32.const_int(u64::from(plan.role().tag()), false).into(),
            i32.const_int(u64::from(root_count), false).into(),
            digest_value(context, types.digest, plan.owner().as_array()).into(),
        ]);
    descriptor.set_constant(true);
    descriptor.set_initializer(&value);
    crate::emission::apply_persistent_linkage(&descriptor, request, true)?;

    Ok(EmittedStrongSafepointRegistrationV1 {
        primary_atom: plan.primary_atom(),
        site: plan.site(),
        descriptor,
    })
}

fn digest_value<'ctx>(
    context: &'ctx Context,
    digest_type: inkwell::types::StructType<'ctx>,
    bytes: &[u8; 32],
) -> StructValue<'ctx> {
    let i8 = context.i8_type();
    let values = bytes
        .iter()
        .map(|byte| i8.const_int(u64::from(*byte), false))
        .collect::<Vec<_>>();
    digest_type.const_named_struct(&[i8.const_array(&values).into()])
}

#[cfg(test)]
mod tests;
