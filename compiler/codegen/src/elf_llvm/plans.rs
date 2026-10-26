//! Reuse planned symbol names across the callable modules of one Cone.

use std::collections::{BTreeMap, HashMap};

use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use llvm_sys::core::{LLVMGetFirstGlobalAlias, LLVMGetNextGlobalAlias};
use scoop_lir::LinkageClass;

use super::*;

#[derive(Clone, Copy)]
struct SymbolPlan {
    order: usize,
    group: usize,
    linkage: LinkageClass,
    dispatch: bool,
}

pub(crate) struct SymbolPlanIndex {
    groups: Vec<String>,
    symbols: HashMap<Vec<u8>, Vec<SymbolPlan>>,
}

impl SymbolPlanIndex {
    pub(crate) fn new(surface: &ObjectSymbolSurfaceV1) -> Self {
        let mut groups = Vec::with_capacity(surface.plans().len());
        let mut symbols: HashMap<Vec<u8>, Vec<SymbolPlan>> = HashMap::new();
        let mut order = 0;
        for (group, plan) in surface.plans().iter().enumerate() {
            groups.push(plan.primary_symbol().symbol().to_string());
            let requests = std::iter::once(plan.primary_symbol()).chain(
                plan.atom_boundaries()
                    .iter()
                    .flat_map(|boundary| [boundary.start(), boundary.end()]),
            );
            for request in requests {
                symbols
                    .entry(request.symbol().as_str().as_bytes().to_vec())
                    .or_default()
                    .push(SymbolPlan {
                        order,
                        group,
                        linkage: request.linkage(),
                        dispatch: plan.definition_role()
                            == scoop_lir::StrongDefinitionRole::DispatchTable,
                    });
                order += 1;
            }
        }
        Self { groups, symbols }
    }

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
        let mut selected = Vec::new();
        let mut select = |value: GlobalValue<'ctx>| {
            if let Some(plans) = self.symbols.get(value.get_name().to_bytes()) {
                selected.extend(plans.iter().map(|plan| (*plan, value)));
            }
        };
        for value in llvm.get_globals() {
            select(value);
        }
        for value in llvm.get_functions() {
            select(value.as_global_value());
        }
        // SAFETY: preparation changes linkage and visibility, never module membership.
        let mut alias = unsafe { LLVMGetFirstGlobalAlias(llvm.as_mut_ptr()) };
        while !alias.is_null() {
            select(unsafe { GlobalValue::new(alias) });
            alias = unsafe { LLVMGetNextGlobalAlias(alias) };
        }
        selected.sort_unstable_by_key(|(plan, _)| plan.order);
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
