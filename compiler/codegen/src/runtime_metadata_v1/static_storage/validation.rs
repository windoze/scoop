use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::targets::TargetData;
use inkwell::types::AnyType;
use inkwell::values::{GlobalValue, UnnamedAddress};
use scoop_lir::{
    LinkageClass, StaticStorageRelocationTableArtifactV1, StrongStaticStorageInitialArtifactPlanV1,
    StrongStaticStorageRegistrationPlanV1,
};

use super::initial_state::{relocation_symbol, template_symbol};
use super::{EMPTY_RELOCATION_SENTINEL, EMPTY_TEMPLATE_SENTINEL, scan_type};
use crate::CodegenError;
use crate::runtime_metadata_v1::RuntimeMetadataV1Types;

#[derive(Clone)]
pub(super) struct PreparedStaticStorageRegistrationV1<'ctx> {
    pub(super) plan: StrongStaticStorageRegistrationPlanV1,
    pub(super) storage_value: GlobalValue<'ctx>,
    pub(super) prior_registration: Option<GlobalValue<'ctx>>,
}

pub(super) fn prepare_registration<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &TargetData,
    types: &RuntimeMetadataV1Types<'ctx>,
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> Result<PreparedStaticStorageRegistrationV1<'ctx>, CodegenError> {
    let semantic = plan.semantic();
    require_strong_linkage("static storage", semantic.symbol())?;
    let storage_symbol = semantic.symbol().symbol();
    if llvm.get_function(storage_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "static storage `{storage_symbol}` collides with an LLVM function"
        )));
    }
    let storage_value = llvm.get_global(storage_symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "static storage `{storage_symbol}` is not defined in the LLVM module"
        ))
    })?;
    let store_size = target_data.get_store_size(&storage_value.get_value_type());
    let abi_alignment = u64::from(target_data.get_abi_alignment(&storage_value.get_value_type()));
    let explicit_alignment = u64::from(storage_value.get_alignment());
    let effective_alignment = if explicit_alignment == 0 {
        abi_alignment
    } else {
        explicit_alignment
    };
    if storage_value.get_linkage() != Linkage::External
        || storage_value.get_unnamed_address() != UnnamedAddress::None
        || storage_value.is_constant()
        || storage_value.is_thread_local()
        || storage_value.get_initializer().is_none()
        || storage_value
            .as_pointer_value()
            .get_type()
            .get_address_space()
            != AddressSpace::default()
        || store_size != semantic.allocation_extent()
        || effective_alignment != semantic.required_alignment()
    {
        return Err(CodegenError(format!(
            "static storage `{storage_symbol}` is not the planned writable address-significant {}/{}/{} definition",
            semantic.byte_size(),
            semantic.allocation_extent(),
            semantic.required_alignment(),
        )));
    }

    require_strong_linkage("static-storage registration", plan.registration_symbol())?;
    let registration_symbol = plan.registration_symbol().symbol();
    if llvm.get_function(registration_symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "static-storage registration `{registration_symbol}` collides with an LLVM function"
        )));
    }
    let prior_registration = llvm.get_global(registration_symbol.as_str());
    if let Some(global) = prior_registration {
        if global.get_value_type() != types.static_storage_descriptor.as_any_type_enum()
            || global.get_linkage() != Linkage::External
            || global.get_unnamed_address() != UnnamedAddress::None
        {
            return Err(CodegenError(format!(
                "static-storage registration `{registration_symbol}` has an incompatible LLVM declaration"
            )));
        }
        if global.get_initializer().is_some() {
            return Err(CodegenError(format!(
                "static-storage registration `{registration_symbol}` is already defined"
            )));
        }
    }

    validate_scan_declaration(context, llvm, plan)?;
    for symbol in plan.immortal_registration_symbols() {
        validate_immortal_registration_declaration(llvm, types, *symbol)?;
    }
    Ok(PreparedStaticStorageRegistrationV1 {
        plan: plan.clone(),
        storage_value,
        prior_registration,
    })
}

pub(super) fn validate_shared_names(
    llvm: &LlvmModule<'_>,
    prepared: &[PreparedStaticStorageRegistrationV1<'_>],
) -> Result<(), CodegenError> {
    if prepared.is_empty() {
        return Ok(());
    }
    for symbol in [EMPTY_TEMPLATE_SENTINEL, EMPTY_RELOCATION_SENTINEL] {
        if llvm.get_global(symbol).is_some() || llvm.get_function(symbol).is_some() {
            return Err(CodegenError(format!(
                "static metadata sentinel `{symbol}` is already present"
            )));
        }
    }
    for prepared in prepared {
        if let StrongStaticStorageInitialArtifactPlanV1::EncodedStaticValue {
            relocation_table,
            ..
        } = prepared.plan.initial_artifacts()
        {
            let mut symbols = vec![template_symbol(&prepared.plan)];
            if matches!(
                relocation_table,
                StaticStorageRelocationTableArtifactV1::Defined { .. }
            ) {
                symbols.push(relocation_symbol(&prepared.plan));
            }
            for symbol in symbols {
                if llvm.get_global(&symbol).is_some() || llvm.get_function(&symbol).is_some() {
                    return Err(CodegenError(format!(
                        "static initial-state support `{symbol}` is already present"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_scan_declaration(
    context: &Context,
    llvm: &LlvmModule<'_>,
    plan: &StrongStaticStorageRegistrationPlanV1,
) -> Result<(), CodegenError> {
    require_strong_linkage("static scan program", plan.scan_symbol())?;
    let symbol = plan.scan_symbol().symbol();
    if llvm.get_function(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "static scan program `{symbol}` collides with an LLVM function"
        )));
    }
    let expected = scan_type(context, plan.semantic().scan_program())?;
    if let Some(global) = llvm.get_global(symbol.as_str())
        && (global.get_value_type() != expected.as_any_type_enum()
            || global.get_linkage() != Linkage::External
            || global.get_unnamed_address() != UnnamedAddress::None
            || global.get_initializer().is_some())
    {
        return Err(CodegenError(format!(
            "static scan program `{symbol}` has an incompatible LLVM declaration"
        )));
    }
    Ok(())
}

fn validate_immortal_registration_declaration(
    llvm: &LlvmModule<'_>,
    types: &RuntimeMetadataV1Types<'_>,
    request: scoop_lir::PersistentSymbolRequest,
) -> Result<(), CodegenError> {
    require_strong_linkage("immortal-object registration", request)?;
    let symbol = request.symbol();
    if llvm.get_function(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "immortal-object registration `{symbol}` collides with an LLVM function"
        )));
    }
    if let Some(global) = llvm.get_global(symbol.as_str())
        && (global.get_value_type() != types.immortal_object_descriptor.as_any_type_enum()
            || global.get_linkage() != Linkage::External
            || global.get_unnamed_address() != UnnamedAddress::None
            || (global.get_initializer().is_some() && !global.is_constant()))
    {
        return Err(CodegenError(format!(
            "immortal-object registration `{symbol}` has an incompatible LLVM declaration"
        )));
    }
    Ok(())
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
