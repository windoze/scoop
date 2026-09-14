use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::AnyType;
use inkwell::values::{GlobalValue, StructValue, UnnamedAddress};
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, PersistentExactTypeId, StrongTypeRegistrationPlanSetV1,
    StrongTypeRegistrationPlanV1,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

const METADATA_ABI_VERSION: u64 = 1;
const TYPE_REGISTRATION_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5054_5950;
const TYPE_REGISTRATION_DESCRIPTOR_SIZE: u64 = 240;
const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET: u64 = 176;
const LAYOUT_FINGERPRINT_OFFSET: u64 = 208;
const DIGEST_SIZE: u64 = 32;

/// One graph-managed digest slot in an emitted type registration.
#[derive(Clone, Copy, Debug)]
pub struct TypeRegistrationPatchSiteV1<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
}

impl<'ctx> TypeRegistrationPatchSiteV1<'ctx> {
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

/// One fully emitted provisional type registration and its three slots.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongTypeRegistrationV1<'ctx> {
    exact_type: PersistentExactTypeId,
    descriptor: GlobalValue<'ctx>,
    registration_definition_patch: TypeRegistrationPatchSiteV1<'ctx>,
    descriptor_definition_patch: TypeRegistrationPatchSiteV1<'ctx>,
    layout_fingerprint_patch: TypeRegistrationPatchSiteV1<'ctx>,
}

impl<'ctx> EmittedStrongTypeRegistrationV1<'ctx> {
    pub const fn exact_type(self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }

    pub const fn registration_definition_patch(self) -> TypeRegistrationPatchSiteV1<'ctx> {
        self.registration_definition_patch
    }

    pub const fn descriptor_definition_patch(self) -> TypeRegistrationPatchSiteV1<'ctx> {
        self.descriptor_definition_patch
    }

    pub const fn layout_fingerprint_patch(self) -> TypeRegistrationPatchSiteV1<'ctx> {
        self.layout_fingerprint_patch
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
/// The three graph-managed digest fields remain zero until finalization.
pub fn emit_strong_type_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongTypeRegistrationPlanSetV1,
) -> Result<EmittedStrongTypeRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let prepared = plan
        .registrations()
        .iter()
        .map(|registration| prepare_registration(llvm, &types, registration))
        .collect::<Result<Vec<_>, _>>()?;
    let registrations = prepared
        .into_iter()
        .map(|registration| emit_registration(context, llvm, &types, registration))
        .collect();
    Ok(EmittedStrongTypeRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

#[derive(Clone, Copy)]
struct PreparedTypeRegistrationV1<'plan, 'ctx> {
    plan: &'plan StrongTypeRegistrationPlanV1,
    type_descriptor: GlobalValue<'ctx>,
    prior_registration: Option<GlobalValue<'ctx>>,
}

fn prepare_registration<'plan, 'ctx>(
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: &'plan StrongTypeRegistrationPlanV1,
) -> Result<PreparedTypeRegistrationV1<'plan, 'ctx>, CodegenError> {
    let registration_request = plan.symbol();
    let registration_symbol = registration_request.symbol();
    if registration_request.linkage() != LinkageClass::ConeStrong {
        return Err(CodegenError(format!(
            "type registration `{registration_symbol}` does not have strong Cone linkage"
        )));
    }

    let descriptor_request = plan.descriptor_symbol();
    let descriptor_symbol = descriptor_request.symbol();
    if descriptor_request.linkage() != LinkageClass::ConeStrong {
        return Err(CodegenError(format!(
            "type descriptor `{descriptor_symbol}` does not have strong Cone linkage"
        )));
    }
    if llvm.get_function(descriptor_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "type descriptor `{descriptor_symbol}` collides with an LLVM function"
        )));
    }
    let type_descriptor = llvm.get_global(descriptor_symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "type descriptor `{descriptor_symbol}` is not declared in the LLVM module"
        ))
    })?;
    if type_descriptor.get_value_type() != types.type_descriptor.as_any_type_enum()
        || type_descriptor.get_linkage() != Linkage::External
        || type_descriptor.get_unnamed_address() != UnnamedAddress::None
        || !type_descriptor.is_constant()
    {
        return Err(CodegenError(format!(
            "type descriptor `{descriptor_symbol}` has an incompatible M23 LLVM declaration"
        )));
    }

    if llvm.get_function(registration_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "type registration `{registration_symbol}` collides with an LLVM function"
        )));
    }
    let prior_registration = llvm.get_global(registration_symbol.as_str());
    if let Some(global) = prior_registration {
        if global.get_value_type() != types.type_registration_descriptor.as_any_type_enum()
            || global.get_linkage() != Linkage::External
            || global.get_unnamed_address() != UnnamedAddress::None
        {
            return Err(CodegenError(format!(
                "type registration `{registration_symbol}` has an incompatible LLVM declaration"
            )));
        }
        if global.get_initializer().is_some() {
            return Err(CodegenError(format!(
                "type registration `{registration_symbol}` is already defined"
            )));
        }
    }

    Ok(PreparedTypeRegistrationV1 {
        plan,
        type_descriptor,
        prior_registration,
    })
}

fn emit_registration<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    prepared: PreparedTypeRegistrationV1<'_, 'ctx>,
) -> EmittedStrongTypeRegistrationV1<'ctx> {
    let plan = prepared.plan;
    let descriptor = prepared.prior_registration.unwrap_or_else(|| {
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
    let zero_digest = types.digest.const_zero();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(TYPE_REGISTRATION_DESCRIPTOR_MAGIC, false)
            .into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(TYPE_REGISTRATION_DESCRIPTOR_SIZE, false)
            .into(),
    ]);
    let identity = types.registration_identity.const_named_struct(&[
        i32.const_int(1, false).into(),
        i32.const_zero().into(),
        digest_value(context, types.digest, plan.exact_type().as_array()).into(),
        zero_digest.into(),
        zero_digest.into(),
        zero_digest.into(),
    ]);
    let value = types.type_registration_descriptor.const_named_struct(&[
        prefix.into(),
        identity.into(),
        i64.const_int(plan.runtime_type().get(), false).into(),
        i64.const_zero().into(),
        prepared.type_descriptor.as_pointer_value().into(),
        zero_digest.into(),
        zero_digest.into(),
    ]);
    descriptor.set_constant(true);
    descriptor.set_initializer(&value);

    let patch = |intent, byte_offset| TypeRegistrationPatchSiteV1 {
        intent,
        definition: plan.definition_plan(),
        atom: plan.primary_atom(),
        owner: descriptor,
        byte_offset,
    };
    EmittedStrongTypeRegistrationV1 {
        exact_type: plan.exact_type(),
        descriptor,
        registration_definition_patch: patch(
            plan.registration_definition_patch(),
            REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
        ),
        descriptor_definition_patch: patch(
            plan.descriptor_definition_patch(),
            DESCRIPTOR_DEFINITION_FINGERPRINT_OFFSET,
        ),
        layout_fingerprint_patch: patch(plan.layout_fingerprint_patch(), LAYOUT_FINGERPRINT_OFFSET),
    }
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
