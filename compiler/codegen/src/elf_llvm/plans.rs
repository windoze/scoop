//! Reuse planned symbol names across the callable modules of one Cone.

use std::collections::{BTreeMap, HashMap};

use llvm_sys::core::{LLVMGetFirstGlobalAlias, LLVMGetNextGlobalAlias};
use scoop_lir::LinkageClass;

use super::*;

#[derive(Clone, Copy)]
struct SymbolPlan {
    order: usize,
    group: usize,
    linkage: LinkageClass,
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
                    });
                order += 1;
            }
        }
        Self { groups, symbols }
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
            let group = &self.groups[plan.group];
            if let Some(previous) = owners.insert(owner.as_value_ref() as usize, plan.group)
                && previous != plan.group
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
