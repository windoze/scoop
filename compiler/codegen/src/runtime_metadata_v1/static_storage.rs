use std::collections::BTreeMap;

use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::ArrayType;
use inkwell::values::GlobalValue;
use scoop_lir::{
    ConeIdentity, PersistentScanId, PersistentStaticStorageId, RefScan,
    StrongStaticStorageRegistrationPlanSetV1, StrongStaticStorageRegistrationPlanV1,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

mod initial_state;
pub use initial_state::{
    EmittedStaticStorageInitialStateV1, EmittedStaticStorageRelocationTableV1,
};
use initial_state::{emit_initial_state, emit_private_constant};

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const STATIC_STORAGE_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5053_544f;
const STATIC_STORAGE_DESCRIPTOR_SIZE: u64 = 200;
const EMPTY_TEMPLATE_SENTINEL: &str = "scoop.metadata.static.template.empty.v1";
const EMPTY_RELOCATION_SENTINEL: &str = "scoop.metadata.static.relocations.empty.v1";

/// One emitted static-storage registration.
#[derive(Clone, Copy, Debug)]
pub struct EmittedStrongStaticStorageRegistrationV1<'ctx> {
    storage: PersistentStaticStorageId,
    descriptor: GlobalValue<'ctx>,
    storage_value: GlobalValue<'ctx>,
    scan_program: GlobalValue<'ctx>,
    initial_state: EmittedStaticStorageInitialStateV1<'ctx>,
}

impl<'ctx> EmittedStrongStaticStorageRegistrationV1<'ctx> {
    pub const fn storage(self) -> PersistentStaticStorageId {
        self.storage
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
pub(crate) fn emit_strong_static_storage_registrations_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &StrongStaticStorageRegistrationPlanSetV1,
) -> Result<EmittedStrongStaticStorageRegistrationSetV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    if plan.registrations().is_empty() {
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
    let registrations = plan
        .registrations()
        .iter()
        .map(|registration| emit_registration(context, llvm, &types, registration, &mut state))
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
    plan: &StrongStaticStorageRegistrationPlanV1,
    state: &mut StaticStorageEmissionStateV1<'ctx>,
) -> Result<EmittedStrongStaticStorageRegistrationV1<'ctx>, CodegenError> {
    let semantic = plan.semantic();
    let symbol = semantic.symbol().symbol();
    let storage_value = llvm.get_global(symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "static storage `{symbol}` is not defined in the LLVM module"
        ))
    })?;
    let scan_program = if let Some(global) = state.scans.get(&semantic.scan()).copied() {
        global
    } else {
        let global = emit_scan_program(context, llvm, plan)?;
        state.scans.insert(semantic.scan(), global);
        global
    };
    let initial_state = emit_initial_state(
        context,
        llvm,
        types,
        plan,
        state.empty_template,
        state.empty_relocations,
        &mut state.immortal_registrations,
    )?;
    let descriptor = llvm
        .get_global(plan.registration_symbol().symbol().as_str())
        .unwrap_or_else(|| {
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
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(STATIC_STORAGE_DESCRIPTOR_MAGIC, false).into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(STATIC_STORAGE_DESCRIPTOR_SIZE, false).into(),
    ]);
    let identity = super::registration_identity::registration_identity_value(
        context,
        types,
        semantic.storage().as_array(),
        plan.definition_owner(),
    );
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
        storage_value.as_pointer_value().into(),
        i64.const_int(semantic.byte_size(), false).into(),
        i64.const_int(semantic.allocation_extent(), false).into(),
        i64.const_int(semantic.required_alignment(), false).into(),
        scan_program.as_pointer_value().into(),
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
    descriptor.set_linkage(super::registration_identity::definition_linkage(
        plan.registration_symbol(),
    )?);
    descriptor.set_initializer(&value);

    Ok(EmittedStrongStaticStorageRegistrationV1 {
        storage: semantic.storage(),
        descriptor,
        storage_value,
        scan_program,
        initial_state,
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
    if plan.semantic().value_layout().local().is_some() {
        global.set_initializer(&value);
        global.set_linkage(super::registration_identity::definition_linkage(
            plan.scan_symbol(),
        )?);
    }
    Ok(global)
}

fn scan_type<'ctx>(
    context: &'ctx Context,
    scan: &RefScan,
) -> Result<ArrayType<'ctx>, CodegenError> {
    let length = match scan {
        RefScan::None => 1,
        RefScan::References(offsets) if !offsets.is_empty() => offsets.len() + 1,
        RefScan::References(_) | RefScan::Sequence(_) | RefScan::Array { .. } => {
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
        RefScan::Sequence(_) | RefScan::Array { .. } => {
            unreachable!("scan_type rejects non-canonical scans")
        }
    })
}

#[cfg(test)]
mod tests;
