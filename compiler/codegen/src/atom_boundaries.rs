//! Strong verifier-boundary aliases for LLVM-owned global atoms.

use std::collections::BTreeMap;
use std::ffi::CString;

use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::targets::TargetData;
use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::core::{
    LLVMAddAlias2, LLVMConstGEP2, LLVMConstInt, LLVMGetModuleContext, LLVMGetNamedGlobalAlias,
    LLVMGetPointerAddressSpace, LLVMGlobalGetValueType, LLVMInt64TypeInContext, LLVMTypeOf,
};
use llvm_sys::prelude::{LLVMTypeRef, LLVMValueRef};
use scoop_lir::{AtomBoundarySymbolsV1, ObjectDefinitionAtomId, ObjectSymbolSurfaceV1};

use crate::CodegenError;

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
    target_data: &TargetData,
    surface: &ObjectSymbolSurfaceV1,
    materializations: impl IntoIterator<Item = GlobalAtomMaterializationV1<'ctx>>,
) -> Result<(), CodegenError> {
    let boundaries = surface
        .plans()
        .iter()
        .flat_map(|plan| plan.atom_boundaries().iter().copied())
        .map(|boundary| (boundary.atom(), boundary))
        .collect::<BTreeMap<_, _>>();
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
        let boundary = boundaries.get(&atom).copied().ok_or_else(|| {
            CodegenError(format!(
                "LLVM global materializes unplanned strong atom {atom}"
            ))
        })?;
        emit_boundary_pair(llvm, target_data, boundary, owner)?;
    }
    Ok(())
}

fn emit_boundary_pair(
    llvm: &LlvmModule<'_>,
    target_data: &TargetData,
    boundary: AtomBoundarySymbolsV1,
    owner: GlobalValue<'_>,
) -> Result<(), CodegenError> {
    if owner.get_initializer().is_none() {
        return Err(CodegenError(format!(
            "strong atom {} owner `{}` is only an LLVM declaration",
            boundary.atom(),
            owner.get_name().to_string_lossy()
        )));
    }
    let size = target_data.get_store_size(&owner.get_value_type());
    if size == 0 {
        return Err(CodegenError(format!(
            "strong atom {} owner `{}` has zero physical extent",
            boundary.atom(),
            owner.get_name().to_string_lossy()
        )));
    }
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
    let existing_alias = unsafe {
        LLVMGetNamedGlobalAlias(llvm.as_mut_ptr(), c_name.as_ptr(), c_name.as_bytes().len())
    };
    if llvm.get_global(name).is_some()
        || llvm.get_function(name).is_some()
        || !existing_alias.is_null()
    {
        return Err(CodegenError(format!(
            "strong atom boundary `{name}` collides with an existing LLVM value"
        )));
    }
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
    path: &std::path::Path,
    target: scoop_lir::LirTargetProfile,
    surface: &ObjectSymbolSurfaceV1,
    definitions: &[scoop_lir::ObjectDefinitionPlanId],
) -> Result<(), CodegenError> {
    let normalization = target.contract().native_symbol_normalization();
    let mut names = Vec::new();
    for definition in definitions {
        let plan = surface
            .plan(*definition)
            .ok_or_else(|| CodegenError(format!("missing atom definition {definition}")))?;
        if plan.primary_symbol().linkage() != scoop_lir::LinkageClass::OdrWeak {
            continue;
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
    let mut bytes = std::fs::read(path)
        .map_err(|error| CodegenError(format!("cannot read atom boundaries: {error}")))?;
    let layout = crate::callable_atom_boundaries::MachOLayout::parse(&bytes)?;
    for name in names {
        layout.materialize_odr_definition(&mut bytes, &name)?;
    }
    std::fs::write(path, bytes)
        .map_err(|error| CodegenError(format!("cannot write atom boundary linkage: {error}")))
}
