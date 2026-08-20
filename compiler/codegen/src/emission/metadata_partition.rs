//! Project one metadata module onto its already planned physical definitions.

use crate::{CodegenError, EmittedStrongRuntimeMetadataV1, target::ValidatedBackendProfile};
use inkwell::module::{Linkage, Module};
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::comdat::LLVMSetComdat;
use llvm_sys::core::{
    LLVMAliasGetAliasee, LLVMGetNamedGlobalAlias, LLVMReplaceAllUsesWith, LLVMSetInitializer,
    LLVMSetModuleInlineAsm2,
};
use scoop_lir::{ObjectDefinitionPlanId, ObjectSymbolSurfaceV1};
use std::collections::BTreeSet;

mod index;

pub(super) struct MetadataModule<'ctx> {
    llvm: Module<'ctx>,
    metadata: EmittedStrongRuntimeMetadataV1,
    definitions: Vec<index::MetadataDefinition>,
}

impl<'ctx> MetadataModule<'ctx> {
    pub(super) fn new(
        llvm: Module<'ctx>,
        metadata: EmittedStrongRuntimeMetadataV1,
        surface: &ObjectSymbolSurfaceV1,
    ) -> Result<Self, CodegenError> {
        let definitions = index::collect(&llvm, surface)?;
        Ok(Self {
            llvm,
            metadata,
            definitions,
        })
    }

    pub(super) fn project(
        &self,
        machine: &TargetMachine,
        profile: ValidatedBackendProfile,
        definitions: &[ObjectDefinitionPlanId],
    ) -> Result<(Module<'ctx>, EmittedStrongRuntimeMetadataV1), CodegenError> {
        let llvm = self.llvm.clone();
        let selected = definitions.iter().copied().collect::<BTreeSet<_>>();
        let mut declarations: Vec<&(String, String)> = Vec::new();
        for entry in &self.definitions {
            if selected.contains(&entry.plan.definition_plan()) {
                continue;
            }
            declarations.extend(&entry.declarations);
            for name in &entry.aliases {
                // SAFETY: this name indexes an alias in the unchanged source and its clone.
                let alias = unsafe {
                    LLVMGetNamedGlobalAlias(llvm.as_mut_ptr(), name.as_ptr(), name.as_bytes().len())
                };
                let pointer = unsafe { LLVMAliasGetAliasee(alias) };
                unsafe { LLVMReplaceAllUsesWith(alias, pointer) };
                unsafe { GlobalValue::new(alias) }.set_linkage(Linkage::Internal);
            }
        }
        // Boundary aliases must be removed while their aliasees are still definitions.
        // Keep names, not LLVM pointers: this pass may also delete private globals.
        global_dce(&llvm, machine)?;
        for (name, declaration) in declarations {
            let Some(global) = llvm.get_global(name) else {
                continue;
            };
            unsafe {
                LLVMSetInitializer(global.as_value_ref(), std::ptr::null_mut());
                LLVMSetComdat(global.as_value_ref(), std::ptr::null_mut());
            }
            global.set_name(declaration);
            global.set_linkage(Linkage::External);
            global.set_visibility(inkwell::GlobalVisibility::Default);
            crate::metadata_sections::set_section(global, "");
        }
        global_dce(&llvm, machine)?;
        // TLS boundaries are the only module assembly in a non-callable module.
        // Recreate them for the selected storage after projecting the LLVM globals.
        unsafe { LLVMSetModuleInlineAsm2(llvm.as_mut_ptr(), c"".as_ptr(), 0) };
        let target_data = machine.get_target_data();
        for entry in &self.definitions {
            if !entry.thread_local || !selected.contains(&entry.plan.definition_plan()) {
                continue;
            }
            let plan = &entry.plan;
            let global = llvm
                .get_global(plan.primary_symbol().symbol().as_str())
                .expect("selected TLS definition");
            super::storage::tls::boundaries(
                &llvm,
                plan,
                plan.primary_symbol().symbol().as_str(),
                target_data.get_store_size(&global.get_value_type()),
                profile.lir_target_profile(),
            )?;
        }
        if profile.lir_target_profile().native_object_format()
            == scoop_lir::NativeObjectFormat::Elf64
        {
            for global in llvm.get_globals().filter(|value| value.is_declaration()) {
                crate::elf_llvm::apply_visibility(global);
            }
            for function in llvm
                .get_functions()
                .filter(|value| value.get_first_basic_block().is_none())
            {
                crate::elf_llvm::apply_visibility(function.as_global_value());
            }
        }
        llvm.verify()
            .map_err(|error| CodegenError(format!("invalid metadata partition: {error}")))?;
        Ok((llvm, self.metadata.select_definitions(definitions)))
    }
}

fn global_dce(llvm: &Module<'_>, machine: &TargetMachine) -> Result<(), CodegenError> {
    llvm.run_passes("globaldce", machine, PassBuilderOptions::create())
        .map_err(|error| CodegenError(format!("cannot partition metadata: {error}")))
}
