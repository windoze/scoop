use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::AnyType;
use inkwell::values::{GlobalValue, StructValue};
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentSafepointSiteId, StrongSafepointRegistrationPlanSetV1,
    StrongSafepointRegistrationPlanV1,
};

use super::{RuntimeMetadataV1Types, registration_identity_value};
use crate::CodegenError;

const METADATA_ABI_VERSION: u64 = 3;
const SAFEPOINT_REGISTRATION_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5053_5054;
const SAFEPOINT_REGISTRATION_DESCRIPTOR_SIZE: u64 = 232;
const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const NORMALIZED_STACKMAP_FINGERPRINT_OFFSET: u64 = 200;
const DIGEST_SIZE: u64 = 32;

/// One graph-managed digest slot in an emitted safepoint registration.
#[derive(Clone, Copy, Debug)]
pub struct SafepointRegistrationPatchSiteV1<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
}

impl<'ctx> SafepointRegistrationPatchSiteV1<'ctx> {
    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }

    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn owner(self) -> GlobalValue<'ctx> {
        self.owner
    }

    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    pub const fn byte_size(self) -> u64 {
        DIGEST_SIZE
    }
}

/// One fully emitted provisional safepoint registration and its two slots.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongSafepointRegistrationV1<'ctx> {
    site: PersistentSafepointSiteId,
    descriptor: GlobalValue<'ctx>,
    registration_definition_patch: SafepointRegistrationPatchSiteV1<'ctx>,
    normalized_stackmap_patch: SafepointRegistrationPatchSiteV1<'ctx>,
}

impl<'ctx> EmittedStrongSafepointRegistrationV1<'ctx> {
    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }

    pub const fn registration_definition_patch(self) -> SafepointRegistrationPatchSiteV1<'ctx> {
        self.registration_definition_patch
    }

    pub const fn normalized_stackmap_patch(self) -> SafepointRegistrationPatchSiteV1<'ctx> {
        self.normalized_stackmap_patch
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
/// plan. Every graph-managed digest field remains zero until finalization.
pub(crate) fn emit_strong_safepoint_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongSafepointRegistrationPlanSetV1,
) -> Result<EmittedStrongSafepointRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let registrations = plan
        .registrations()
        .iter()
        .map(|registration| emit_registration(context, llvm, &types, *registration))
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
) -> Result<EmittedStrongSafepointRegistrationV1<'ctx>, CodegenError> {
    let request = plan.symbol();
    let symbol = request.symbol();
    if llvm.get_function(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "safepoint registration `{symbol}` collides with an LLVM function"
        )));
    }
    let descriptor = if let Some(global) = llvm.get_global(symbol.as_str()) {
        if global.get_value_type() != types.safepoint_registration_descriptor.as_any_type_enum()
            || global.get_linkage() != Linkage::External
        {
            return Err(CodegenError(format!(
                "safepoint registration `{symbol}` has an incompatible LLVM declaration"
            )));
        }
        if global.get_initializer().is_some() {
            return Err(CodegenError(format!(
                "safepoint registration `{symbol}` is already defined"
            )));
        }
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
    let zero_digest = types.digest.const_zero();
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
            i32.const_int(u64::from(plan.root_pair_count()), false)
                .into(),
            digest_value(context, types.digest, plan.owner().as_array()).into(),
            zero_digest.into(),
        ]);
    descriptor.set_constant(true);
    descriptor.set_initializer(&value);
    crate::emission::apply_persistent_linkage(&descriptor, request, true)?;

    Ok(EmittedStrongSafepointRegistrationV1 {
        site: plan.site(),
        descriptor,
        registration_definition_patch: SafepointRegistrationPatchSiteV1 {
            intent: plan.registration_definition_patch(),
            definition: plan.definition_plan(),
            atom: plan.primary_atom(),
            owner: descriptor,
            byte_offset: REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
        },
        normalized_stackmap_patch: SafepointRegistrationPatchSiteV1 {
            intent: plan.normalized_stackmap_patch(),
            definition: plan.definition_plan(),
            atom: plan.primary_atom(),
            owner: descriptor,
            byte_offset: NORMALIZED_STACKMAP_FINGERPRINT_OFFSET,
        },
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
