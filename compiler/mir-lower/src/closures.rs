use super::*;

mod anonymous;
mod lambdas;
mod references;
mod reuse;
use reuse::{ClosureDefinition, ClosureDefinitions};

impl Lowerer {
    /// Materialize the concrete closure classes before lowering any body, so
    /// every creation expression resolves directly to a typed class id.
    pub(super) fn declare_closures(&mut self, module: &hir::Module) {
        let mut definitions = ClosureDefinitions::default();
        self.declare_lambdas(module, &mut definitions);
        self.declare_anonymous(module, &mut definitions);
        self.declare_references(module, &mut definitions);
    }

    pub(super) fn lower_reference_callee(
        &mut self,
        _module: &hir::Module,
        callable: hir::CallableReferenceCallee,
    ) -> mir::Callee {
        let callable = match callable {
            hir::CallableReferenceCallee::Local(callable) => callable,
            hir::CallableReferenceCallee::Imported(callee) => {
                return mir::Callee::External(
                    self.imported_dependency_callable_map[&callee].callable,
                );
            }
        };
        let hir::Callable::Function(function) = callable;
        self.instances.get(function).map_or_else(
            || mir::Callee::User(self.function_map[&function]),
            mir::Callee::Monomorphized,
        )
    }

    pub(super) fn bound_reference_call_kind(
        &mut self,
        module: &hir::Module,
        _receiver_ty: hir::TypeId,
        callable: hir::CallableReferenceCallee,
    ) -> mir::CallKind {
        let callable = match callable {
            hir::CallableReferenceCallee::Local(callable) => callable,
            hir::CallableReferenceCallee::Imported(callee) => {
                return match module.imported_dependency_callables[callee].dispatch() {
                    scoop_hir::ImportedDependencyDispatch::Direct => mir::CallKind::Direct,
                    scoop_hir::ImportedDependencyDispatch::Virtual { slot } => {
                        mir::CallKind::Virtual { slot }
                    }
                    scoop_hir::ImportedDependencyDispatch::Interface { interface, slot } => {
                        let (id, _) = module
                            .interfaces
                            .iter()
                            .find(|(_, declaration)| {
                                declaration.origin.concrete_type_id() == Some(interface)
                            })
                            .expect("a bound dependency reference retains its interface receiver");
                        mir::CallKind::Interface {
                            interface: self.interfaces.mir_id(id),
                            slot,
                        }
                    }
                };
            }
        };
        let function = module.callable_function(callable);
        let declaration = &module.functions[function];
        let method = declaration
            .receiver
            .method()
            .expect("a bound member reference names method metadata");
        match method.dispatch {
            hir::MethodDispatch::Direct | hir::MethodDispatch::FinalOverride(_) => {
                mir::CallKind::Direct
            }
            hir::MethodDispatch::Virtual(family) => {
                let hir::TypeKind::Class(class) = module.types[method.owner].kind else {
                    unreachable!("a virtual family belongs to a class method")
                };
                let slot = self.method_slots[&self.class_map[&class]][&family];
                mir::CallKind::Virtual { slot }
            }
            hir::MethodDispatch::Interface { interface, slot } => mir::CallKind::Interface {
                interface: self.interfaces.mir_id(interface),
                slot: slot.into_raw(),
            },
        }
    }
}

fn capture_source(index: usize) -> mir::ClosureFieldSource {
    mir::ClosureFieldSource::Capture {
        declaration_index: u32::try_from(index)
            .expect("a closure capture declaration index fits the identity schema"),
    }
}

fn order_closure_fields(
    identity: &mir::ClosureEnvironmentIdentity,
    fields: Vec<(mir::ClosureFieldSource, mir::Field)>,
) -> Vec<mir::Field> {
    let mut by_source = fields.into_iter().collect::<HashMap<_, _>>();
    let physical = identity
        .fields()
        .iter()
        .map(|field| {
            by_source
                .remove(&field.source())
                .expect("every persistent closure field has one physical definition")
        })
        .collect();
    assert!(
        by_source.is_empty(),
        "every physical closure field has a persistent identity"
    );
    physical
}
