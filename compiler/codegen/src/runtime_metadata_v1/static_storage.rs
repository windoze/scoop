use std::collections::BTreeMap;

use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::targets::TargetData;
use inkwell::types::ArrayType;
use inkwell::values::{GlobalValue, StructValue};
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, GlobalId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentScanId, PersistentStaticStorageId, RefScan, StrongStaticStorageRegistrationPlanSetV1,
    StrongStaticStorageRegistrationPlanV1,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

mod initial_state;
pub use initial_state::{
    EmittedStaticStorageInitialStateV1, EmittedStaticStorageRelocationTableV1,
};
use initial_state::{emit_initial_state, emit_private_constant};

mod validation;
use validation::{
    PreparedStaticStorageRegistrationV1, prepare_registration, validate_shared_names,
};

const METADATA_ABI_VERSION: u64 = 1;
const STATIC_STORAGE_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5053_544f;
const STATIC_STORAGE_DESCRIPTOR_SIZE: u64 = 296;
const REGISTRATION_DEFINITION_FINGERPRINT_OFFSET: u64 = 120;
const SCAN_FINGERPRINT_OFFSET: u64 = 200;
const LAYOUT_FINGERPRINT_OFFSET: u64 = 232;
const DIGEST_SIZE: u64 = 32;
const EMPTY_TEMPLATE_SENTINEL: &str = "scoop.metadata.static.template.empty.v1";
const EMPTY_RELOCATION_SENTINEL: &str = "scoop.metadata.static.relocations.empty.v1";

/// One graph-managed digest slot in an emitted static-storage registration.
#[derive(Clone, Copy, Debug)]
pub struct StaticStorageRegistrationPatchSiteV1<'ctx> {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
    byte_offset: u64,
}

impl<'ctx> StaticStorageRegistrationPatchSiteV1<'ctx> {
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

/// One fully emitted provisional static-storage registration and its three
/// digest slots.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongStaticStorageRegistrationV1<'ctx> {
    storage: PersistentStaticStorageId,
    global: GlobalId,
    descriptor: GlobalValue<'ctx>,
    storage_value: GlobalValue<'ctx>,
    scan_program: GlobalValue<'ctx>,
    initial_state: EmittedStaticStorageInitialStateV1<'ctx>,
    registration_definition_patch: StaticStorageRegistrationPatchSiteV1<'ctx>,
    scan_fingerprint_patch: StaticStorageRegistrationPatchSiteV1<'ctx>,
    layout_fingerprint_patch: StaticStorageRegistrationPatchSiteV1<'ctx>,
}

impl<'ctx> EmittedStrongStaticStorageRegistrationV1<'ctx> {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
    }

    pub const fn global(self) -> GlobalId {
        self.global
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }

    pub const fn storage_value(self) -> GlobalValue<'ctx> {
        self.storage_value
    }

    pub const fn scan_program(self) -> GlobalValue<'ctx> {
        self.scan_program
    }

    pub const fn initial_state(self) -> EmittedStaticStorageInitialStateV1<'ctx> {
        self.initial_state
    }

    pub const fn registration_definition_patch(self) -> StaticStorageRegistrationPatchSiteV1<'ctx> {
        self.registration_definition_patch
    }

    pub const fn scan_fingerprint_patch(self) -> StaticStorageRegistrationPatchSiteV1<'ctx> {
        self.scan_fingerprint_patch
    }

    pub const fn layout_fingerprint_patch(self) -> StaticStorageRegistrationPatchSiteV1<'ctx> {
        self.layout_fingerprint_patch
    }
}

/// Canonically ordered emission result for every static storage in one Cone.
#[derive(Clone, Debug)]
pub struct EmittedStrongStaticStorageRegistrationSetV1<'ctx> {
    producer: ConeIdentity,
    registrations: Vec<EmittedStrongStaticStorageRegistrationV1<'ctx>>,
}

impl<'ctx> EmittedStrongStaticStorageRegistrationSetV1<'ctx> {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn registrations(&self) -> &[EmittedStrongStaticStorageRegistrationV1<'ctx>] {
        &self.registrations
    }
}

/// Emit every strong static-storage registration from its closed LIR plan.
/// All graph-managed digest fields remain zero until finalization.
pub fn emit_strong_static_storage_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &TargetData,
    plan: &StrongStaticStorageRegistrationPlanSetV1,
) -> Result<EmittedStrongStaticStorageRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let prepared = plan
        .registrations()
        .iter()
        .map(|registration| prepare_registration(context, llvm, target_data, &types, registration))
        .collect::<Result<Vec<_>, _>>()?;
    validate_shared_names(llvm, &prepared)?;

    if prepared.is_empty() {
        return Ok(EmittedStrongStaticStorageRegistrationSetV1 {
            producer: plan.producer(),
            registrations: Vec::new(),
        });
    }

    let empty_template = emit_private_constant(
        llvm,
        EMPTY_TEMPLATE_SENTINEL,
        context.i8_type().const_zero().into(),
    );
    let empty_relocations = emit_private_constant(
        llvm,
        EMPTY_RELOCATION_SENTINEL,
        types.static_immortal_relocation.const_zero().into(),
    );
    let mut state = StaticStorageEmissionStateV1 {
        empty_template,
        empty_relocations,
        scans: BTreeMap::new(),
        immortal_registrations: BTreeMap::new(),
    };
    let registrations = prepared
        .into_iter()
        .map(|prepared| emit_registration(context, llvm, &types, prepared, &mut state))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EmittedStrongStaticStorageRegistrationSetV1 {
        producer: plan.producer(),
        registrations,
    })
}

struct StaticStorageEmissionStateV1<'ctx> {
    empty_template: GlobalValue<'ctx>,
    empty_relocations: GlobalValue<'ctx>,
    scans: BTreeMap<PersistentScanId, GlobalValue<'ctx>>,
    immortal_registrations: BTreeMap<String, GlobalValue<'ctx>>,
}

fn emit_registration<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    types: &RuntimeMetadataV1Types<'ctx>,
    prepared: PreparedStaticStorageRegistrationV1<'ctx>,
    state: &mut StaticStorageEmissionStateV1<'ctx>,
) -> Result<EmittedStrongStaticStorageRegistrationV1<'ctx>, CodegenError> {
    let plan = prepared.plan;
    let semantic = plan.semantic();
    let scan_program = if let Some(global) = state.scans.get(&semantic.scan()).copied() {
        global
    } else {
        let global = emit_scan_program(context, llvm, &plan)?;
        state.scans.insert(semantic.scan(), global);
        global
    };
    let initial_state = emit_initial_state(
        context,
        llvm,
        types,
        &plan,
        state.empty_template,
        state.empty_relocations,
        &mut state.immortal_registrations,
    )?;
    let descriptor = prepared.prior_registration.unwrap_or_else(|| {
        let global = llvm.add_global(
            types.static_storage_descriptor,
            None,
            plan.registration_symbol().symbol().as_str(),
        );
        global.set_linkage(Linkage::External);
        global
    });

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let zero_digest = types.digest.const_zero();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(STATIC_STORAGE_DESCRIPTOR_MAGIC, false).into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(STATIC_STORAGE_DESCRIPTOR_SIZE, false).into(),
    ]);
    let identity = types.registration_identity.const_named_struct(&[
        i32.const_int(1, false).into(),
        i32.const_zero().into(),
        digest_value(context, types.digest, semantic.storage().as_array()).into(),
        zero_digest.into(),
        zero_digest.into(),
        zero_digest.into(),
    ]);
    let template_span = types.byte_span.const_named_struct(&[
        initial_state.template().as_pointer_value().into(),
        i64.const_int(
            semantic.initial_state().initial_template().len() as u64,
            false,
        )
        .into(),
    ]);
    let value = types.static_storage_descriptor.const_named_struct(&[
        prefix.into(),
        identity.into(),
        i32.const_int(u64::from(semantic.scan_kind().tag()), false)
            .into(),
        i32.const_int(u64::from(semantic.initial_state().tag()), false)
            .into(),
        prepared.storage_value.as_pointer_value().into(),
        i64.const_int(semantic.byte_size(), false).into(),
        i64.const_int(semantic.allocation_extent(), false).into(),
        i64.const_int(semantic.required_alignment(), false).into(),
        scan_program.as_pointer_value().into(),
        zero_digest.into(),
        zero_digest.into(),
        template_span.into(),
        initial_state
            .relocations()
            .global()
            .as_pointer_value()
            .into(),
        i64.const_int(
            semantic.initial_state().immortal_relocations().len() as u64,
            false,
        )
        .into(),
    ]);
    descriptor.set_constant(true);
    descriptor.set_initializer(&value);

    let patch = |intent, byte_offset| StaticStorageRegistrationPatchSiteV1 {
        intent,
        definition: plan.registration_definition_plan(),
        atom: plan.registration_primary_atom(),
        owner: descriptor,
        byte_offset,
    };
    Ok(EmittedStrongStaticStorageRegistrationV1 {
        storage: semantic.storage(),
        global: semantic.global(),
        descriptor,
        storage_value: prepared.storage_value,
        scan_program,
        initial_state,
        registration_definition_patch: patch(
            plan.registration_definition_patch(),
            REGISTRATION_DEFINITION_FINGERPRINT_OFFSET,
        ),
        scan_fingerprint_patch: patch(plan.scan_fingerprint_patch(), SCAN_FINGERPRINT_OFFSET),
        layout_fingerprint_patch: patch(plan.layout_fingerprint_patch(), LAYOUT_FINGERPRINT_OFFSET),
    })
}

fn emit_scan_program<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> Result<GlobalValue<'ctx>, CodegenError> {
    let words = scan_words(context, plan.semantic().scan_program())?;
    let value = context.i64_type().const_array(&words);
    let symbol = plan.scan_symbol().symbol();
    let global = llvm.get_global(symbol.as_str()).unwrap_or_else(|| {
        let global = llvm.add_global(value.get_type(), None, symbol.as_str());
        global.set_linkage(Linkage::External);
        global
    });
    global.set_constant(true);
    global.set_alignment(8);
    global.set_initializer(&value);
    Ok(global)
}

fn scan_type<'ctx>(
    context: &'ctx Context,
    scan: &RefScan,
) -> Result<ArrayType<'ctx>, CodegenError> {
    let length = match scan {
        RefScan::None => 1,
        RefScan::References(offsets) if !offsets.is_empty() => offsets.len() + 1,
        RefScan::References(_) | RefScan::Sequence(_) => {
            return Err(CodegenError(
                "static scan plan is not in canonical None/References form".to_string(),
            ));
        }
    };
    let length = u32::try_from(length)
        .map_err(|_| CodegenError("static scan program exceeds LLVM array bounds".to_string()))?;
    Ok(context.i64_type().array_type(length))
}

fn scan_words<'ctx>(
    context: &'ctx Context,
    scan: &RefScan,
) -> Result<Vec<inkwell::values::IntValue<'ctx>>, CodegenError> {
    scan_type(context, scan)?;
    Ok(match scan {
        RefScan::None => vec![context.i64_type().const_zero()],
        RefScan::References(offsets) => {
            std::iter::once(context.i64_type().const_int(offsets.len() as u64, false))
                .chain(
                    offsets
                        .iter()
                        .map(|offset| context.i64_type().const_int(*offset, false)),
                )
                .collect()
        }
        RefScan::Sequence(_) => unreachable!("scan_type rejects non-canonical sequences"),
    })
}

fn digest_value<'ctx>(
    context: &'ctx Context,
    digest_type: inkwell::types::StructType<'ctx>,
    bytes: &[u8; 32],
) -> StructValue<'ctx> {
    let values = bytes
        .iter()
        .map(|byte| context.i8_type().const_int(u64::from(*byte), false))
        .collect::<Vec<_>>();
    digest_type.const_named_struct(&[context.i8_type().const_array(&values).into()])
}

#[cfg(test)]
mod tests;
