//! Apply ELF visibility and COMDAT ownership from the existing symbol plans.

use std::collections::BTreeMap;
use std::ffi::CString;

use inkwell::GlobalVisibility;
use inkwell::module::{Linkage, Module};
use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::core::{
    LLVMAliasGetAliasee, LLVMGetConstOpcode, LLVMGetFirstUse, LLVMGetNamedGlobalAlias,
    LLVMGetOperand, LLVMIsAConstantExpr, LLVMIsAGlobalAlias, LLVMIsAGlobalObject,
};
use scoop_lir::{LinkageClass, ObjectSymbolSurfaceV1};

use crate::CodegenError;

pub(crate) fn prepare(
    llvm: &Module<'_>,
    surface: &ObjectSymbolSurfaceV1,
) -> Result<(), CodegenError> {
    let mut owners = BTreeMap::new();
    for plan in surface.plans() {
        let group_name = plan.primary_symbol().symbol();
        let requests = std::iter::once(plan.primary_symbol()).chain(
            plan.atom_boundaries()
                .iter()
                .flat_map(|boundary| [boundary.start(), boundary.end()]),
        );
        for request in requests {
            let name = request.symbol();
            let Some(value) = global(llvm, name.as_str())? else {
                continue;
            };
            apply_visibility(value);
            if value.is_declaration() {
                continue;
            }
            let owner = definition_owner(value)?;
            let local_owner = matches!(owner.get_linkage(), Linkage::Private | Linkage::Internal);
            owner.set_visibility(if local_owner {
                GlobalVisibility::Default
            } else {
                GlobalVisibility::Hidden
            });
            if request.linkage() != LinkageClass::OdrWeak {
                continue;
            }
            value.set_linkage(Linkage::WeakODR);
            if !local_owner {
                owner.set_linkage(Linkage::WeakODR);
            }
            if let Some(previous) = owners.insert(owner.as_value_ref() as usize, group_name.clone())
                && previous != group_name
            {
                return Err(CodegenError(format!(
                    "ELF global belongs to two definition groups: {previous} and {group_name}"
                )));
            }
            owner.set_comdat(llvm.get_or_insert_comdat(group_name.as_str()));
        }
    }
    Ok(())
}

pub(crate) fn apply_visibility(value: GlobalValue<'_>) {
    // Unused hidden declarations emit NOTYPE symbols even without relocations.
    // Besides conflicting with TLS definitions, those symbols make a shared
    // ODR body depend on unrelated declarations in its consumer's module.
    let unused =
        value.is_declaration() && unsafe { LLVMGetFirstUse(value.as_value_ref()) }.is_null();
    value.set_visibility(if unused {
        GlobalVisibility::Default
    } else {
        GlobalVisibility::Hidden
    });
}

fn global<'ctx>(
    llvm: &Module<'ctx>,
    name: &str,
) -> Result<Option<GlobalValue<'ctx>>, CodegenError> {
    if let Some(value) = llvm.get_global(name) {
        return Ok(Some(value));
    }
    if let Some(value) = llvm.get_function(name) {
        return Ok(Some(value.as_global_value()));
    }
    let name = CString::new(name).map_err(|_| CodegenError("ELF symbol contains NUL".into()))?;
    // SAFETY: the module and name storage remain alive throughout the lookup.
    let raw =
        unsafe { LLVMGetNamedGlobalAlias(llvm.as_mut_ptr(), name.as_ptr(), name.as_bytes().len()) };
    Ok((!raw.is_null()).then(|| unsafe { GlobalValue::new(raw) }))
}

fn definition_owner(value: GlobalValue<'_>) -> Result<GlobalValue<'_>, CodegenError> {
    let mut raw = value.as_value_ref();
    // These aliases are emitted by atom_boundaries: a direct owner or its end
    // GEP. Preserve that exact relation when assigning the ELF section group.
    unsafe {
        if !LLVMIsAGlobalAlias(raw).is_null() {
            raw = LLVMAliasGetAliasee(raw);
        }
        if !LLVMIsAConstantExpr(raw).is_null() {
            match LLVMGetConstOpcode(raw) {
                llvm_sys::LLVMOpcode::LLVMGetElementPtr | llvm_sys::LLVMOpcode::LLVMBitCast => {
                    raw = LLVMGetOperand(raw, 0)
                }
                _ => {
                    return Err(CodegenError(
                        "ELF atom alias is not a direct owner/extent".into(),
                    ));
                }
            }
        }
        if LLVMIsAGlobalObject(raw).is_null() {
            return Err(CodegenError(
                "ELF atom alias does not resolve to an LLVM global object".into(),
            ));
        }
        Ok(GlobalValue::new(raw))
    }
}
