//! Emit boundary aliases for LLVM-owned global atoms.

use std::collections::BTreeMap;
use std::ffi::CString;

use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::core::{
    LLVMAddAlias2, LLVMConstGEP2, LLVMConstInt, LLVMGetModuleContext, LLVMGetPointerAddressSpace,
    LLVMGlobalGetValueType, LLVMInt64TypeInContext, LLVMTypeOf,
};
use llvm_sys::prelude::{LLVMTypeRef, LLVMValueRef};
use scoop_lir::{AtomBoundarySymbolsV1, ObjectDefinitionAtomId, ObjectSymbolSurfaceV1};

use crate::CodegenError;

mod coff;
pub(crate) mod elf;

#[derive(Clone, Copy, Debug)]
pub(crate) struct GlobalAtomMaterializationV1<'ctx> {
    atom: ObjectDefinitionAtomId,
    owner: GlobalValue<'ctx>,
}

impl<'ctx> GlobalAtomMaterializationV1<'ctx> {
    pub(crate) const fn new(atom: ObjectDefinitionAtomId, owner: GlobalValue<'ctx>) -> Self {
        Self { atom, owner }
    }
}

pub(crate) fn emit_global_atom_boundaries_v1<'ctx>(
    llvm: &LlvmModule<'ctx>,
    surface: &ObjectSymbolSurfaceV1,
    materializations: impl IntoIterator<Item = GlobalAtomMaterializationV1<'ctx>>,
) -> Result<(), CodegenError> {
    let mut atoms = BTreeMap::new();
    for materialization in materializations {
        match atoms.entry(materialization.atom) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(materialization.owner);
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if *entry.get() == materialization.owner => {}
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(CodegenError(format!(
                    "strong atom {} has multiple LLVM global owners",
                    materialization.atom
                )));
            }
        }
    }

    for (atom, owner) in atoms {
        let boundary = surface.atom_boundary(atom).ok_or_else(|| {
            CodegenError(format!(
                "LLVM global materializes unplanned strong atom {atom}"
            ))
        })?;
        emit_boundary_pair(llvm, boundary, owner)?;
    }
    Ok(())
}

fn emit_boundary_pair(
    llvm: &LlvmModule<'_>,
    boundary: AtomBoundarySymbolsV1,
    owner: GlobalValue<'_>,
) -> Result<(), CodegenError> {
    let owner_ref = owner.as_value_ref();
    let value_type = unsafe { LLVMGlobalGetValueType(owner_ref) };
    let address_space = unsafe { LLVMGetPointerAddressSpace(LLVMTypeOf(owner_ref)) };
    let one = unsafe {
        let context = LLVMGetModuleContext(llvm.as_mut_ptr());
        LLVMConstInt(LLVMInt64TypeInContext(context), 1, 0)
    };
    let mut indices = [one];
    let end = unsafe { LLVMConstGEP2(value_type, owner_ref, indices.as_mut_ptr(), 1) };
    let start = boundary.start().symbol();
    if owner.get_name().to_bytes() != start.as_str().as_bytes() {
        add_external_alias(llvm, boundary.start(), value_type, address_space, owner_ref)?;
    }
    add_external_alias(llvm, boundary.end(), value_type, address_space, end)?;
    Ok(())
}

fn add_external_alias(
    llvm: &LlvmModule<'_>,
    request: scoop_lir::PersistentSymbolRequest,
    value_type: LLVMTypeRef,
    address_space: u32,
    aliasee: LLVMValueRef,
) -> Result<(), CodegenError> {
    let symbol = request.symbol();
    let name = symbol.as_str();
    let c_name = CString::new(name)
        .map_err(|_| CodegenError("strong atom boundary contains NUL".to_string()))?;
    let alias = unsafe {
        LLVMAddAlias2(
            llvm.as_mut_ptr(),
            value_type,
            address_space,
            aliasee,
            c_name.as_ptr(),
        )
    };
    let alias = unsafe { GlobalValue::new(alias) };
    // LLVM 22 Mach-O lowers weak aliases to local weak references. Keep the
    // exact section aliases external here and set their final weak-definition
    // flags alongside the other native atom-boundary materializations.
    alias.set_linkage(Linkage::External);
    Ok(())
}

pub(crate) fn materialize_global_linkages_v1(
    bytes: &mut Vec<u8>,
    target: scoop_lir::LirTargetProfile,
    surface: &ObjectSymbolSurfaceV1,
    definitions: &[scoop_lir::ObjectDefinitionPlanId],
) -> Result<(), CodegenError> {
    if target.native_object_format() == scoop_lir::NativeObjectFormat::Elf64 {
        return elf::materialize(bytes, surface, definitions);
    }
    if target.native_object_format() == scoop_lir::NativeObjectFormat::Coff64 {
        return coff::materialize(bytes, surface, definitions);
    }
    let normalization = target.contract().native_symbol_normalization();
    let mut names = Vec::new();
    for definition in definitions {
        let plan = surface
            .plan(*definition)
            .ok_or_else(|| CodegenError(format!("missing atom definition {definition}")))?;
        if plan.primary_symbol().linkage() != scoop_lir::LinkageClass::OdrWeak {
            continue;
        }
        if plan.definition_role() == scoop_lir::StrongDefinitionRole::DispatchTable {
            // A nonzero-offset interface view is an external LLVM alias too.
            names.push(
                normalization
                    .compiler_generated_object_symbol(plan.primary_symbol().symbol().as_str())
                    .into_bytes(),
            );
        }
        for boundary in plan.atom_boundaries() {
            if plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody
                && !matches!(
                    boundary.atom_role(),
                    scoop_lir::DefinitionAtomRole::RuntimeRecord
                        | scoop_lir::DefinitionAtomRole::AddressTakenConstant
                )
            {
                continue;
            }
            for request in [boundary.start(), boundary.end()] {
                names.push(
                    normalization
                        .compiler_generated_object_symbol(request.symbol().as_str())
                        .into_bytes(),
                );
            }
        }
    }
    if names.is_empty() {
        return Ok(());
    }
    let layout = crate::callable_atom_boundaries::MachOLayout::parse(bytes, target)?;
    for name in names {
        layout.materialize_odr_definition(bytes, &name)?;
    }
    Ok(())
}
