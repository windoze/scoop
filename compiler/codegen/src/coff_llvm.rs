//! Give COFF definitions independent contributions before machine emission.

use std::collections::BTreeMap;

use inkwell::comdat::ComdatSelectionKind;
use inkwell::module::{Linkage, Module};
use inkwell::values::AsValueRef;
use llvm_sys::core::LLVMIsAGlobalAlias;
use scoop_lir::LinkageClass;

use crate::CodegenError;
use crate::symbol_plan::SymbolPlanIndex;

pub(crate) fn prepare(llvm: &Module<'_>, symbols: &SymbolPlanIndex) -> Result<(), CodegenError> {
    let selected = symbols.selected(llvm);
    let values = selected
        .iter()
        .map(|(_, value)| (value.get_name().to_bytes().to_vec(), *value))
        .collect::<BTreeMap<_, _>>();
    let mut owners = BTreeMap::new();
    for (plan, value) in selected {
        if value.is_declaration() {
            continue;
        }
        let owner = crate::llvm_definitions::definition_owner(value)?;
        let primary = symbols.primary(owner.get_name().to_bytes()).unwrap_or(plan);
        let primary_name = &symbols.groups[primary.group];
        let group = if values
            .get(primary_name.as_bytes())
            .is_some_and(|value| !value.is_declaration())
        {
            primary_name.clone()
        } else {
            // Callable modules can contain image-owned constants without the
            // descriptor. Their first emitted boundary names the contribution.
            owners
                .get(&(owner.as_value_ref() as usize))
                .cloned()
                .unwrap_or_else(|| value.get_name().to_string_lossy().into_owned())
        };
        if let Some(previous) = owners.insert(owner.as_value_ref() as usize, group.clone())
            && previous != group
        {
            return Err(CodegenError(format!(
                "COFF storage belongs to both {previous} and {group}"
            )));
        }
        // Real section definitions carry COMDAT selection, not weak fallback.
        if unsafe { !LLVMIsAGlobalAlias(value.as_value_ref()).is_null() } {
            value.set_linkage(Linkage::External);
        }
        let comdat = llvm.get_or_insert_comdat(&group);
        comdat.set_selection_kind(match primary.linkage {
            LinkageClass::OdrWeak => ComdatSelectionKind::Any,
            LinkageClass::ConeStrong | LinkageClass::TemplateSupportHidden => {
                ComdatSelectionKind::NoDuplicates
            }
            LinkageClass::RuntimeAbi => {
                return Err(CodegenError(
                    "runtime declaration cannot own compiler storage".into(),
                ));
            }
        });
        owner.set_comdat(comdat);
    }
    Ok(())
}
