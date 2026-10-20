use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::types::{FunctionType, StructType};
use inkwell::values::{FunctionValue, GlobalValue};
use scoop_lir::{
    EntryProductionPlanV1, ExecutableEntryPlanV1, ObjectDefinitionPlanId, PersistentSymbolRequest,
};

use super::RuntimeMetadataV1Types;
use crate::CodegenError;

const METADATA_ABI_VERSION: u64 = scoop_lir::RUNTIME_METADATA_ABI_VERSION_V1 as u64;
const ROOT_ENTRY_DESCRIPTOR_MAGIC: u64 = 0x5343_4f4f_5045_4e54;
const ROOT_ENTRY_DESCRIPTOR_SIZE: u64 = 128;

/// Emitted executable root descriptor.
#[derive(Clone, Copy, Debug)]
pub struct EmittedRootEntryV1<'ctx> {
    definition: ObjectDefinitionPlanId,
    descriptor: GlobalValue<'ctx>,
}

impl<'ctx> EmittedRootEntryV1<'ctx> {
    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn descriptor(self) -> GlobalValue<'ctx> {
        self.descriptor
    }
}

/// Closed emission result preserving the library/executable distinction.
#[derive(Clone, Copy, Debug)]
pub enum EmittedEntryProductionV1<'ctx> {
    Library,
    Executable(EmittedRootEntryV1<'ctx>),
}

/// Emit the canonical runtime metadata v1 entry surface.
///
/// A library plan emits no root symbol. An executable plan defines exactly
/// one complete root descriptor. The gateway body and failure-root registration may be
/// defined before or after this call, but their declarations must have the
/// exact ABI types and strong Cone linkage.
pub(crate) fn emit_entry_production_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &EntryProductionPlanV1,
) -> Result<EmittedEntryProductionV1<'ctx>, CodegenError> {
    match plan {
        EntryProductionPlanV1::Library => Ok(EmittedEntryProductionV1::Library),
        EntryProductionPlanV1::Executable(plan) => {
            emit_root_entry_v1(context, llvm, plan).map(EmittedEntryProductionV1::Executable)
        }
    }
}

fn emit_root_entry_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: &ExecutableEntryPlanV1,
) -> Result<EmittedRootEntryV1<'ctx>, CodegenError> {
    let types = RuntimeMetadataV1Types::new(context);
    let descriptor_request = plan.root_descriptor_symbol();
    let descriptor_symbol = descriptor_request.symbol();
    let failure_root_request = plan.failure_root_registration_symbol().map_err(|error| {
        CodegenError(format!("derive failure-root registration symbol: {error}"))
    })?;
    let failure_root =
        declare_strong_record(llvm, types.static_storage_descriptor, failure_root_request);

    let gateway_request = plan
        .gateway_symbol()
        .map_err(|error| CodegenError(format!("derive root gateway symbol: {error}")))?;
    let pointer = context.ptr_type(inkwell::AddressSpace::default());
    let gateway_type = context.i32_type().fn_type(
        &[context.i32_type().into(), pointer.into(), pointer.into()],
        false,
    );
    let gateway = declare_strong_gateway(llvm, gateway_type, gateway_request);

    let i32 = context.i32_type();
    let i64 = context.i64_type();
    let prefix = types.descriptor_prefix.const_named_struct(&[
        i64.const_int(ROOT_ENTRY_DESCRIPTOR_MAGIC, false).into(),
        i32.const_int(METADATA_ABI_VERSION, false).into(),
        i32.const_int(ROOT_ENTRY_DESCRIPTOR_SIZE, false).into(),
    ]);
    let descriptor_value = types.root_entry_descriptor.const_named_struct(&[
        prefix.into(),
        digest_value(context, types.digest, plan.root_cone().as_array()).into(),
        digest_value(context, types.digest, plan.main().body().as_array()).into(),
        digest_value(context, types.digest, plan.gateway().as_array()).into(),
        failure_root.as_pointer_value().into(),
        gateway.as_global_value().as_pointer_value().into(),
    ]);
    let descriptor = llvm.add_global(
        types.root_entry_descriptor,
        None,
        descriptor_symbol.as_str(),
    );
    descriptor.set_linkage(Linkage::External);
    descriptor.set_constant(true);
    descriptor.set_initializer(&descriptor_value);

    Ok(EmittedRootEntryV1 {
        definition: plan.root_descriptor_definition(),
        descriptor,
    })
}

fn declare_strong_record<'ctx>(
    llvm: &LlvmModule<'ctx>,
    record_type: StructType<'ctx>,
    request: PersistentSymbolRequest,
) -> GlobalValue<'ctx> {
    llvm.get_global(request.symbol().as_str())
        .unwrap_or_else(|| {
            let record = llvm.add_global(record_type, None, request.symbol().as_str());
            record.set_linkage(Linkage::External);
            record
        })
}

fn declare_strong_gateway<'ctx>(
    llvm: &LlvmModule<'ctx>,
    gateway_type: FunctionType<'ctx>,
    request: PersistentSymbolRequest,
) -> FunctionValue<'ctx> {
    llvm.get_function(request.symbol().as_str())
        .unwrap_or_else(|| {
            let gateway = llvm.add_function(request.symbol().as_str(), gateway_type, None);
            gateway.set_linkage(Linkage::External);
            gateway
        })
}

fn digest_value<'ctx>(
    context: &'ctx Context,
    digest_type: StructType<'ctx>,
    bytes: &[u8; 32],
) -> inkwell::values::StructValue<'ctx> {
    let i8 = context.i8_type();
    let values = bytes
        .iter()
        .map(|byte| i8.const_int(u64::from(*byte), false))
        .collect::<Vec<_>>();
    digest_type.const_named_struct(&[i8.const_array(&values).into()])
}

#[cfg(test)]
mod tests;
