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
            for method in &decl.methods {
                let (family, target) = match *method {
                    hir::ClassMethod::Local(function) => {
                        let Some(method) = module.functions[function].receiver.method() else {
                            continue;
                        };
                        let family = match method.dispatch {
                            hir::MethodDispatch::Virtual(family)
                            | hir::MethodDispatch::FinalOverride(family) => family,
                            hir::MethodDispatch::Direct | hir::MethodDispatch::Interface { .. } => {
                                continue;
                            }
                        };
                        (
                            family,
                            mir::TableSlot::Function(self.function_map[&function]),
                        )
                    }
                    hir::ClassMethod::Imported { family, callable } => (
                        family,
                        mir::TableSlot::External(
                            self.imported_dependency_callable_map[&callable].scoop_entry(),
                        ),
                    ),
                };
                match slots.get(&family) {
                    Some(&slot) => vtable[slot as usize] = target,
                    None => {
                        slots.insert(family, vtable.len() as u32);
                        vtable.push(target);
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
                            hir::InterfaceImplementationTarget::Method(function)
                            | hir::InterfaceImplementationTarget::Abstract {
                                declaration: function,
                            } => mir::TableSlot::Function(self.function_map[&function]),
                            hir::InterfaceImplementationTarget::Imported(callable)
                            | hir::InterfaceImplementationTarget::ImportedAbstract {
                                declaration: callable,
                            } => mir::TableSlot::External(
                                self.imported_dependency_callable_map[&callable].scoop_entry(),
                            ),
                        };
                        let slot = method.slot.into_raw() as usize;
                        let previous = slots[slot].replace(target);
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
