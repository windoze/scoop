use super::*;

impl Lowerer {
    /// Fix every class's vtable and itables (impl spec 2.9): a derived
    /// Build each class's vtable / itables from dispatch identities emitted by
    /// concrete HIR. Vtable slots are inherited base-prefix first; overrides
    /// carry the base virtual family and replace its slot, while overloads
    /// carry distinct families. Interface records and every slot target are
    /// likewise complete upstream data, never reconstructed from signatures.
    pub(crate) fn compute_dispatch(&mut self, module: &hir::Module, order: &[hir::ClassId]) {
        for &hir_id in order {
            let mir_id = self.class_map[&hir_id];
            let decl = &module.classes[hir_id];
            let (mut vtable, mut slots) = match decl.base_class() {
                Some(base) => {
                    let base = self.class_map[&base];
                    (
                        clone_slots(&self.classes[base].vtable),
                        self.method_slots[&base].clone(),
                    )
                }
                None => (Vec::new(), HashMap::new()),
            };
            for &fn_id in &decl.methods {
                let function = &module.functions[fn_id];
                let Some(method) = function.receiver.method() else {
                    continue;
                };
                let family = match method.dispatch {
                    hir::MethodDispatch::Virtual(family)
                    | hir::MethodDispatch::FinalOverride(family) => family,
                    hir::MethodDispatch::Direct | hir::MethodDispatch::Interface { .. } => {
                        continue;
                    }
                };
                let mir_fn = self.function_map[&fn_id];
                match slots.get(&family) {
                    Some(&slot) => vtable[slot as usize] = mir::TableSlot::Function(mir_fn),
                    None => {
                        slots.insert(family, vtable.len() as u32);
                        vtable.push(mir::TableSlot::Function(mir_fn));
                    }
                }
            }
            let itables = decl
                .interface_implementations
                .iter()
                .map(|implementation| {
                    let interface = self.interfaces.mir_id(implementation.interface);
                    let method_count = module.interfaces[implementation.interface].methods.len();
                    let mut slots = std::iter::repeat_with(|| None)
                        .take(method_count)
                        .collect::<Vec<_>>();
                    for method in &implementation.methods {
                        let target = match method.target {
                            hir::InterfaceImplementationTarget::Method(function) => function,
                            hir::InterfaceImplementationTarget::Abstract { declaration } => {
                                declaration
                            }
                        };
                        let slot = method.slot.into_raw() as usize;
                        let previous = slots[slot]
                            .replace(mir::TableSlot::Function(self.function_map[&target]));
                        assert!(
                            previous.is_none(),
                            "concrete HIR emits each itable slot once"
                        );
                    }
                    let slots = slots
                        .into_iter()
                        .map(|slot| slot.expect("concrete HIR emits every itable slot"))
                        .collect();
                    mir::ItableRecord { interface, slots }
                })
                .collect();
            let class = &mut self.classes[mir_id];
            class.vtable = vtable;
            class.itables = itables;
            self.method_slots.insert(mir_id, slots);
        }
    }
}
