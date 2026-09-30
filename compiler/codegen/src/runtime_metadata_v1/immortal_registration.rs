use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::AnyType;
use inkwell::values::{GlobalValue, UnnamedAddress};
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionPlanId, PersistentImmortalObjectId, StrongImmortalObjectRegistrationPlanSetV1,
    StrongImmortalObjectRegistrationPlanV1,
};

use super::RuntimeMetadataV1Types;
use crate::{CodegenError, ManagedAddressSpace};

const METADATA_ABI_VERSION: u64 = 2;
const IMMORTAL_OBJECT_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5049_4d4d;
const IMMORTAL_OBJECT_DESCRIPTOR_SIZE: u64 = 184;
const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const DIGEST_SIZE: u64 = 32;

/// The graph-managed definition slot in an emitted immortal-object registration.
#[derive(Clone, Copy, Debug)]
pub struct ImmortalObjectRegistrationPatchSiteV1<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
}

impl<'ctx> ImmortalObjectRegistrationPatchSiteV1<'ctx> {
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

/// One fully emitted provisional immortal-object registration.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongImmortalObjectRegistrationV1<'ctx> {
    object: PersistentImmortalObjectId,
    descriptor: GlobalValue<'ctx>,
    object_value: GlobalValue<'ctx>,
    type_registration: GlobalValue<'ctx>,
    registration_definition_patch: ImmortalObjectRegistrationPatchSiteV1<'ctx>,
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

    pub const fn registration_definition_patch(
        self,
    ) -> ImmortalObjectRegistrationPatchSiteV1<'ctx> {
        self.registration_definition_patch
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
/// The graph-managed definition fingerprint remains zero until finalization.
pub(crate) fn emit_strong_immortal_object_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongImmortalObjectRegistrationPlanSetV1,
) -> Result<EmittedStrongImmortalObjectRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let prepared = plan
        .registrations()
        .iter()
        .map(|registration| prepare_registration(llvm, &types, *registration))
        .collect::<Result<Vec<_>, _>>()?;
    let registrations = prepared
        .into_iter()
        .map(|registration| emit_registration(context, llvm, &types, registration))
        .collect();
    Ok(EmittedStrongImmortalObjectRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

#[derive(Clone, Copy)]
struct PreparedImmortalObjectRegistrationV1<'ctx> {
    plan: StrongImmortalObjectRegistrationPlanV1,
    object_value: GlobalValue<'ctx>,
    prior_type_registration: Option<GlobalValue<'ctx>>,
    prior_registration: Option<GlobalValue<'ctx>>,
}

fn prepare_registration<'ctx>(
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: StrongImmortalObjectRegistrationPlanV1,
) -> Result<PreparedImmortalObjectRegistrationV1<'ctx>, CodegenError> {
    let object_request = plan.object_symbol();
    let object_symbol = object_request.symbol();
    let expected_linkage = plan.definition_owner().linkage();
    if object_request.linkage() != expected_linkage {
        return Err(CodegenError(
            "immortal object and registration linkage disagree".to_string(),
        ));
    }
    if llvm.get_function(object_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "immortal object `{object_symbol}` collides with an LLVM function"
        )));
    }
    let object_value = llvm.get_global(object_symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "immortal object `{object_symbol}` is not defined in the LLVM module"
        ))
    })?;
    let object_linkage = if expected_linkage == LinkageClass::OdrWeak {
        Linkage::WeakODR
    } else {
        Linkage::External
    };
    if object_value.get_linkage() != object_linkage
        || object_value.get_unnamed_address() != UnnamedAddress::None
        || !object_value.is_constant()
        || object_value.get_initializer().is_none()
    {
        return Err(CodegenError(format!(
            "immortal object `{object_symbol}` is not a constant address-significant definition with its planned linkage"
        )));
    }
    if object_value
        .as_pointer_value()
        .get_type()
        .get_address_space()
        != ManagedAddressSpace::MOVING_GC.inkwell()
    {
        return Err(CodegenError(format!(
            "immortal object `{object_symbol}` is not in the managed LLVM address space"
        )));
    }

    let type_request = plan.type_registration_symbol();
    let type_symbol = type_request.symbol();
    require_strong_linkage("type registration", type_request)?;
    if llvm.get_function(type_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "type registration `{type_symbol}` collides with an LLVM function"
        )));
    }
    let prior_type_registration = llvm.get_global(type_symbol.as_str());
    if let Some(global) = prior_type_registration {
        if global.get_value_type() != types.type_registration_descriptor.as_any_type_enum()
            || global.get_linkage() != Linkage::External
            || global.get_unnamed_address() != UnnamedAddress::None
            || (global.get_initializer().is_some() && !global.is_constant())
        {
            return Err(CodegenError(format!(
                "type registration `{type_symbol}` has an incompatible LLVM declaration"
            )));
        }
    }

    let registration_request = plan.registration_symbol();
    let registration_symbol = registration_request.symbol();
    if registration_request.linkage() != expected_linkage {
        return Err(CodegenError(
            "immortal registration linkage disagrees with its definition".to_string(),
        ));
    }
    if llvm.get_function(registration_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "immortal-object registration `{registration_symbol}` collides with an LLVM function"
        )));
    }
    let prior_registration = llvm.get_global(registration_symbol.as_str());
    if let Some(global) = prior_registration {
        if global.get_value_type() != types.immortal_object_descriptor.as_any_type_enum()
            || global.get_linkage() != Linkage::External
            || global.get_unnamed_address() != UnnamedAddress::None
        {
            return Err(CodegenError(format!(
                "immortal-object registration `{registration_symbol}` has an incompatible LLVM declaration"
            )));
        }
        if global.get_initializer().is_some() {
            return Err(CodegenError(format!(
                "immortal-object registration `{registration_symbol}` is already defined"
            )));
        }
    }

    Ok(PreparedImmortalObjectRegistrationV1 {
        plan,
        object_value,
        prior_type_registration,
        prior_registration,
    })
}

fn require_strong_linkage(
    kind: &str,
    request: scoop_lir::PersistentSymbolRequest,
) -> Result<(), CodegenError> {
    if request.linkage() == LinkageClass::ConeStrong {
        Ok(())
    } else {
        Err(CodegenError(format!(
            "{kind} `{}` does not have strong Cone linkage",
            request.symbol()
        )))
    }
}

fn emit_registration<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    prepared: PreparedImmortalObjectRegistrationV1<'ctx>,
) -> EmittedStrongImmortalObjectRegistrationV1<'ctx> {
    let plan = prepared.plan;
    let type_registration = prepared.prior_type_registration.unwrap_or_else(|| {
        llvm.get_global(plan.type_registration_symbol().symbol().as_str())
            .unwrap_or_else(|| {
                let global = llvm.add_global(
                    types.type_registration_descriptor,
                    None,
                    plan.type_registration_symbol().symbol().as_str(),
                );
                global.set_linkage(Linkage::External);
                global
            })
    });
    let descriptor = prepared.prior_registration.unwrap_or_else(|| {
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
    let object_start = prepared
        .object_value
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

    EmittedStrongImmortalObjectRegistrationV1 {
        object: plan.object(),
        descriptor,
        object_value: prepared.object_value,
        type_registration,
        registration_definition_patch: ImmortalObjectRegistrationPatchSiteV1 {
            intent: plan.registration_definition_patch(),
            definition: plan.registration_definition_plan(),
            atom: plan.registration_primary_atom(),
            owner: descriptor,
            byte_offset: REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
        },
    }
}

#[cfg(test)]
mod tests;
