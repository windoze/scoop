//! Index metadata ownership once, before cloning physical members.

use super::*;
use llvm_sys::core::{
    LLVMGetConstOpcode, LLVMGetOperand, LLVMIsAConstantExpr, LLVMIsAGlobalVariable,
};
use std::collections::BTreeMap;
use std::ffi::CString;

pub(super) struct MetadataDefinition {
    pub plan: scoop_lir::DefinitionSymbolPlanV1,
    pub declarations: Vec<(String, String)>,
    pub aliases: Vec<CString>,
    pub thread_local: bool,
}

pub(super) fn collect(
    llvm: &Module<'_>,
    surface: &ObjectSymbolSurfaceV1,
) -> Result<Vec<MetadataDefinition>, CodegenError> {
    let mut entries = Vec::new();
    for plan in surface.plans() {
        let mut declarations = BTreeMap::new();
        let mut aliases = Vec::new();
        let primary = plan.primary_symbol().symbol();
        if let Some(global) = llvm.get_global(primary.as_str())
            && global.get_initializer().is_some()
        {
            declarations.insert(primary.to_string(), primary.to_string());
        }
        for boundary in plan.atom_boundaries() {
            let start = boundary.start().symbol();
            for request in [boundary.start(), boundary.end()] {
                let name = request.symbol();
                let c_name = CString::new(name.as_str())
                    .map_err(|_| CodegenError("metadata symbol contains NUL".into()))?;
                // SAFETY: these names and aliases belong to the prepared source module.
                let raw = unsafe {
                    LLVMGetNamedGlobalAlias(
                        llvm.as_mut_ptr(),
                        c_name.as_ptr(),
                        c_name.as_bytes().len(),
                    )
                };
                if !raw.is_null() {
                    let pointer = unsafe { LLVMAliasGetAliasee(raw) };
                    let owner = global_owner(llvm, pointer)?;
                    let owner_name = owner.get_name().to_string_lossy().into_owned();
                    declarations.entry(owner_name.clone()).or_insert_with(|| {
                        if owner_name == primary.as_str() {
                            owner_name
                        } else {
                            start.to_string()
                        }
                    });
                    aliases.push(c_name);
                } else if let Some(global) = llvm.get_global(name.as_str())
                    && global.get_initializer().is_some()
                {
                    declarations
                        .entry(name.to_string())
                        .or_insert_with(|| start.to_string());
                }
            }
        }
        if !declarations.is_empty() {
            let thread_local = llvm
                .get_global(plan.primary_symbol().symbol().as_str())
                .is_some_and(|global| global.is_thread_local());
            entries.push(MetadataDefinition {
                plan: plan.clone(),
                declarations: declarations.into_iter().collect(),
                aliases,
                thread_local,
            });
        }
    }
    Ok(entries)
}
fn global_owner<'ctx>(
    _: &Module<'ctx>,
    raw: llvm_sys::prelude::LLVMValueRef,
) -> Result<GlobalValue<'ctx>, CodegenError> {
    let mut raw = raw;
    // SAFETY: these aliases were emitted by atom_boundaries as an owner or its end GEP.
    unsafe {
        if !LLVMIsAConstantExpr(raw).is_null() {
            match LLVMGetConstOpcode(raw) {
                llvm_sys::LLVMOpcode::LLVMGetElementPtr | llvm_sys::LLVMOpcode::LLVMBitCast => {
                    raw = LLVMGetOperand(raw, 0);
                }
                _ => {
                    return Err(CodegenError(
                        "metadata boundary is not an owner/extent".into(),
                    ));
                }
            }
        }
        if LLVMIsAGlobalVariable(raw).is_null() {
            return Err(CodegenError(
                "metadata boundary does not resolve to a global".into(),
            ));
        }
        Ok(GlobalValue::new(raw))
    }
}
