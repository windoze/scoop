//! Runtime checks that the selected LLVM backend still satisfies the closed
//! physical-layout contract carried by LIR.

use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::targets::{TargetData, TargetMachine};
use inkwell::types::AnyType;
use scoop_lir::{BackendScalarKind, ScalarLayout};

use super::ValidatedBackendProfile;
use crate::CodegenError;

pub(super) fn validate_target_machine(
    profile: ValidatedBackendProfile,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    let contract = profile.lir_target_selection.target();
    let target_data = machine.get_target_data();
    let data_layout = target_data.get_data_layout();
    let actual_data_layout = data_layout.as_str().to_str().map_err(|error| {
        CodegenError(format!(
            "LLVM data layout for {} is not UTF-8: {error}",
            profile.canonical_triple
        ))
    })?;
    if actual_data_layout != contract.canonical_llvm_data_layout() {
        return Err(CodegenError(format!(
            "LLVM data layout for {} is {actual_data_layout:?}, expected {:?}",
            profile.canonical_triple,
            contract.canonical_llvm_data_layout(),
        )));
    }

    let context = Context::create();
    for (name, kind, ty) in [
        (
            "i1",
            BackendScalarKind::I1,
            context.bool_type().as_any_type_enum(),
        ),
        (
            "i8",
            BackendScalarKind::I8,
            context.i8_type().as_any_type_enum(),
        ),
        (
            "i16",
            BackendScalarKind::I16,
            context.i16_type().as_any_type_enum(),
        ),
        (
            "i32",
            BackendScalarKind::I32,
            context.i32_type().as_any_type_enum(),
        ),
        (
            "i64",
            BackendScalarKind::I64,
            context.i64_type().as_any_type_enum(),
        ),
    ] {
        validate_llvm_type_layout(&target_data, name, &ty, contract.scalar_layout(kind))?;
    }

    validate_llvm_pointer_layout(
        &target_data,
        &context,
        "AS0 data",
        AddressSpace::default(),
        contract.data_pointer().layout(),
    )?;
    validate_llvm_pointer_layout(
        &target_data,
        &context,
        "AS0 code",
        AddressSpace::default(),
        contract.code_pointer().layout(),
    )?;
    validate_llvm_pointer_layout(
        &target_data,
        &context,
        "AS0 metadata",
        AddressSpace::default(),
        contract.metadata_pointer_layout(),
    )?;
    validate_llvm_pointer_layout(
        &target_data,
        &context,
        "AS1 managed",
        profile.managed_address_space.inkwell(),
        contract.managed_pointer_layout(),
    )?;
    crate::runtime_metadata_v1::RuntimeMetadataV1Types::new(&context)
        .validate_layout(&target_data)?;
    Ok(())
}

fn validate_llvm_type_layout(
    target_data: &TargetData,
    name: &str,
    ty: &dyn AnyType<'_>,
    expected: ScalarLayout,
) -> Result<(), CodegenError> {
    let actual_size = target_data.get_store_size(ty);
    let actual_alignment = u64::from(target_data.get_abi_alignment(ty));
    if actual_size == expected.size_bytes() && actual_alignment == expected.alignment_bytes() {
        return Ok(());
    }
    Err(CodegenError(format!(
        "LLVM {name} layout is {actual_size}-byte size/{actual_alignment}-byte alignment, expected {}-byte size/{}-byte alignment",
        expected.size_bytes(),
        expected.alignment_bytes(),
    )))
}

fn validate_llvm_pointer_layout(
    target_data: &TargetData,
    context: &Context,
    name: &str,
    address_space: AddressSpace,
    expected: ScalarLayout,
) -> Result<(), CodegenError> {
    let pointer = context.ptr_type(address_space);
    let actual_size = u64::from(target_data.get_pointer_byte_size(Some(address_space)));
    let actual_alignment = u64::from(target_data.get_abi_alignment(&pointer));
    if actual_size == expected.size_bytes() && actual_alignment == expected.alignment_bytes() {
        return Ok(());
    }
    Err(CodegenError(format!(
        "LLVM {name} pointer layout is {actual_size}-byte size/{actual_alignment}-byte alignment, expected {}-byte size/{}-byte alignment",
        expected.size_bytes(),
        expected.alignment_bytes(),
    )))
}
