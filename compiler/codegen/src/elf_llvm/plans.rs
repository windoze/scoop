//! Apply ELF rules to the shared Cone symbol index.

use std::collections::BTreeMap;

use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use scoop_lir::LinkageClass;

#[cfg(test)]
use crate::symbol_plan::SymbolPlan;
use crate::symbol_plan::SymbolPlanIndex;

use super::*;

impl SymbolPlanIndex {
    pub(crate) fn prepare_callable(
        &self,
        llvm: &Module<'_>,
        machine: &TargetMachine,
    ) -> Result<(), CodegenError> {
        // Function passes can leave dead constant users of declarations. GlobalDCE
        // removes those users and unused declarations, including external symbols
        // absent from this Cone's definition plans. Metadata partitions already
        // perform this cleanup when projecting their definitions.
        llvm.run_passes("globaldce", machine, PassBuilderOptions::create())
            .map_err(|error| CodegenError(format!("ELF declaration cleanup failed: {error}")))?;
        self.prepare(llvm)
    }

    pub(crate) fn prepare<'ctx>(&self, llvm: &Module<'ctx>) -> Result<(), CodegenError> {
        let selected = self.selected(llvm);
        let mut owners = BTreeMap::new();
        for (plan, value) in selected {
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
            if plan.linkage != LinkageClass::OdrWeak {
                continue;
            }
            value.set_linkage(Linkage::WeakODR);
            if !local_owner {
                owner.set_linkage(Linkage::WeakODR);
            }
            let group_index = if plan.dispatch {
                self.symbols
                    .get(owner.get_name().to_bytes())
                    .and_then(|plans| plans.iter().find(|plan| plan.dispatch))
                    .map_or(plan.group, |plan| plan.group)
            } else {
                plan.group
            };
            let group = &self.groups[group_index];
            if let Some(previous) = owners.insert(owner.as_value_ref() as usize, group_index)
                && previous != group_index
            {
                return Err(CodegenError(format!(
                    "ELF global belongs to two definition groups: {} and {group}",
                    self.groups[previous]
                )));
            }
            owner.set_comdat(llvm.get_or_insert_comdat(group));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
