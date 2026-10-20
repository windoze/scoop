use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::values::GlobalValue;
use scoop_lir::{
    ConeIdentity, LinkageClass, PersistentImmortalObjectId,
    StrongImmortalObjectRegistrationPlanSetV1, StrongImmortalObjectRegistrationPlanV1,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const IMMORTAL_OBJECT_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4d4d;
const IMMORTAL_OBJECT_DESCRIPTOR_SIZE: u64 = 152;

/// One fully emitted immortal-object registration.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongImmortalObjectRegistrationV1<'ctx> {
    object: PersistentImmortalObjectId,
    descriptor: GlobalValue<'ctx>,
    object_value: GlobalValue<'ctx>,
    type_registration: GlobalValue<'ctx>,
}

impl<'ctx> EmittedStrongImmortalObjectRegistrationV1<'ctx> {
    pub const fn object(self) -> PersistentImmortalObjectId {
        self.object
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }

    pub const fn object_value(self) -> GlobalValue<'ctx> {
        self.object_value
    }

    pub const fn type_registration(self) -> GlobalValue<'ctx> {
        self.type_registration
    }
}

/// Canonically ordered emission result for every immortal object in one Cone.
#[derive(Clone, Debug)]
pub struct EmittedStrongImmortalObjectRegistrationSetV1<'ctx> {
    producer: ConeIdentity,
    registrations: Vec<EmittedStrongImmortalObjectRegistrationV1<'ctx>>,
}

impl<'ctx> EmittedStrongImmortalObjectRegistrationSetV1<'ctx> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[EmittedStrongImmortalObjectRegistrationV1<'ctx>] {
        &self.registrations
    }
}

/// Emit every strong immortal-object registration from the closed LIR plan.
pub(crate) fn emit_strong_immortal_object_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongImmortalObjectRegistrationPlanSetV1,
) -> Result<EmittedStrongImmortalObjectRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let registrations = plan
        .registrations()
        .iter()
        .map(|registration| emit_registration(context, llvm, &types, *registration))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmittedStrongImmortalObjectRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

fn emit_registration<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<EmittedStrongImmortalObjectRegistrationV1<'ctx>, CodegenError> {
    let object_symbol = plan.object_symbol().symbol();
    let object_value = llvm.get_global(object_symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "immortal object `{object_symbol}` is not defined in the LLVM module"
        ))
    })?;
    let type_registration = llvm
        .get_global(plan.type_registration_symbol().symbol().as_str())
        .unwrap_or_else(|| {
            let global = llvm.add_global(
                types.type_registration_descriptor,
                None,
                plan.type_registration_symbol().symbol().as_str(),
            );
            global.set_linkage(Linkage::External);
            global
        });
    let descriptor = llvm
        .get_global(plan.registration_symbol().symbol().as_str())
        .unwrap_or_else(|| {
            let global = llvm.add_global(
                types.immortal_object_descriptor,
                None,
                plan.registration_symbol().symbol().as_str(),
            );
            global.set_linkage(Linkage::External);
            global
        });

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(IMMORTAL_OBJECT_DESCRIPTOR_MAGIC, false)
            .into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(IMMORTAL_OBJECT_DESCRIPTOR_SIZE, false).into(),
    ]);
    let identity = super::registration_identity::registration_identity_value(
        context,
        types,
        plan.object().as_array(),
        plan.definition_owner(),
    );
    let object_start = object_value
        .as_pointer_value()
        .const_address_space_cast(context.ptr_type(AddressSpace::default()));
    let value = types.immortal_object_descriptor.const_named_struct(&[
        prefix.into(),
        identity.into(),
        object_start.into(),
        i64.const_int(plan.object_size(), false).into(),
        i64.const_int(plan.required_alignment(), false).into(),
        type_registration.as_pointer_value().into(),
    ]);
    descriptor.set_constant(true);
    descriptor.set_linkage(
        if plan.definition_owner().linkage() == LinkageClass::OdrWeak {
            Linkage::WeakODR
        } else {
            Linkage::External
        },
    );
    descriptor.set_initializer(&value);

    Ok(EmittedStrongImmortalObjectRegistrationV1 {
        object: plan.object(),
        descriptor,
        object_value,
        type_registration,
    })
}

#[cfg(test)]
mod tests;
