use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::AnyType;
use inkwell::values::{GlobalValue, UnnamedAddress};
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, PersistentCallableBodyId, StrongCallableRegistrationPlanSetV1,
    StrongCallableRegistrationPlanV1,
};

use super::{RuntimeMetadataV1Types, registration_identity_value};
use crate::CodegenError;

mod context_keys;

const METADATA_ABI_VERSION: u64 = 4;
const CALLABLE_REGISTRATION_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5043_414c;
const CALLABLE_REGISTRATION_DESCRIPTOR_SIZE: u64 = 208;
const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const BODY_DEFINITION_FINGERPRINT_OFFSET: u64 = 152;
const DIGEST_SIZE: u64 = 32;

/// One graph-managed digest slot in an emitted callable registration.
#[derive(Clone, Copy, Debug)]
pub struct CallableRegistrationPatchSiteV1<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
}

impl<'ctx> CallableRegistrationPatchSiteV1<'ctx> {
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

/// One fully emitted provisional callable registration and its two slots.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongCallableRegistrationV1<'ctx> {
    body: PersistentCallableBodyId,
    descriptor: GlobalValue<'ctx>,
    registration_definition_patch: CallableRegistrationPatchSiteV1<'ctx>,
    body_definition_patch: CallableRegistrationPatchSiteV1<'ctx>,
}

impl<'ctx> EmittedStrongCallableRegistrationV1<'ctx> {
    pub const fn body(self) -> PersistentCallableBodyId {
        self.body
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }

    pub const fn registration_definition_patch(self) -> CallableRegistrationPatchSiteV1<'ctx> {
        self.registration_definition_patch
    }

    pub const fn body_definition_patch(self) -> CallableRegistrationPatchSiteV1<'ctx> {
        self.body_definition_patch
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
/// plan. Both graph-managed digest fields remain zero until finalization.
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
    if llvm.get_global(entry_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "callable entry `{entry_symbol}` collides with an LLVM global"
        )));
    }
    let entry = llvm.get_function(entry_symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "callable entry `{entry_symbol}` is not declared in the LLVM module"
        ))
    })?;
    // The registration is emitted before its selected body's basic blocks.
    // Definition linkage already comes from the body plan.
    let entry_linkage = if entry_request.linkage() == LinkageClass::OdrWeak {
        Linkage::WeakODR
    } else {
        Linkage::External
    };
    if entry.get_linkage() != entry_linkage {
        return Err(CodegenError(format!(
            "callable entry `{entry_symbol}` does not have external linkage"
        )));
    }
    if entry.as_global_value().get_unnamed_address() != UnnamedAddress::None {
        return Err(CodegenError(format!(
            "callable entry `{entry_symbol}` is not address-significant"
        )));
    }

    if llvm.get_function(descriptor_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "callable registration `{descriptor_symbol}` collides with an LLVM function"
        )));
    }
    let descriptor = if let Some(global) = llvm.get_global(descriptor_symbol.as_str()) {
        if global.get_value_type() != types.callable_registration_descriptor.as_any_type_enum()
            || global.get_linkage() != Linkage::External
            || global.get_unnamed_address() != UnnamedAddress::None
        {
            return Err(CodegenError(format!(
                "callable registration `{descriptor_symbol}` has an incompatible LLVM declaration"
            )));
        }
        if global.get_initializer().is_some() {
            return Err(CodegenError(format!(
                "callable registration `{descriptor_symbol}` is already defined"
            )));
        }
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
    let zero_digest = types.digest.const_zero();
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
        zero_digest.into(),
        entry.as_global_value().as_pointer_value().into(),
        keys.into(),
        i64.const_int(key_count, false).into(),
    ]);
    descriptor.set_constant(true);
    descriptor.set_initializer(&value);
    crate::emission::apply_persistent_linkage(&descriptor, descriptor_request, true)?;

    Ok(EmittedStrongCallableRegistrationV1 {
        body: plan.body(),
        descriptor,
        registration_definition_patch: CallableRegistrationPatchSiteV1 {
            intent: plan.registration_definition_patch(),
            definition: plan.definition_plan(),
            atom: plan.primary_atom(),
            owner: descriptor,
            byte_offset: REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
        },
        body_definition_patch: CallableRegistrationPatchSiteV1 {
            intent: plan.body_definition_patch(),
            definition: plan.definition_plan(),
            atom: plan.primary_atom(),
            owner: descriptor,
            byte_offset: BODY_DEFINITION_FINGERPRINT_OFFSET,
        },
    })
}

#[cfg(test)]
mod tests;
