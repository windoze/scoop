//! Reuse planned symbol names across the LLVM modules of one Cone.

use std::collections::HashMap;

use inkwell::module::Module;
use inkwell::values::GlobalValue;
use llvm_sys::core::{LLVMGetFirstGlobalAlias, LLVMGetNextGlobalAlias};
use scoop_lir::{LinkageClass, ObjectSymbolSurfaceV1};

#[derive(Clone, Copy)]
pub(crate) struct SymbolPlan {
    pub order: usize,
    pub group: usize,
    pub linkage: LinkageClass,
    pub dispatch: bool,
}

pub(crate) struct SymbolPlanIndex {
    pub groups: Vec<String>,
    pub symbols: HashMap<Vec<u8>, Vec<SymbolPlan>>,
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

    pub(crate) fn primary(&self, name: &[u8]) -> Option<SymbolPlan> {
        self.symbols
            .get(name)?
            .iter()
            .find(|plan| self.groups[plan.group].as_bytes() == name)
            .copied()
    }

    pub(crate) fn selected<'ctx>(
        &self,
        llvm: &Module<'ctx>,
    ) -> Vec<(SymbolPlan, GlobalValue<'ctx>)> {
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
        // Preparation changes linkage/COMDAT, never membership or symbol names.
        let mut alias = unsafe { LLVMGetFirstGlobalAlias(llvm.as_mut_ptr()) };
        while !alias.is_null() {
            select(unsafe { GlobalValue::new(alias) });
            alias = unsafe { LLVMGetNextGlobalAlias(alias) };
        }
        selected.sort_unstable_by_key(|(plan, _)| plan.order);
        selected
    }
}
