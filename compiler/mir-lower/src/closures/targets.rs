use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReferenceDefinition {
    Primitive(scoop_hir::PrimitiveMemberIntrinsic),
    MaybeUninit(hir::MaybeUninitIntrinsic),
    Direct(mir::Callee),
    Bound(BoundReferenceTarget),
}

/// A method reference before its captured receiver has been loaded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BoundReferenceTarget {
    Direct(mir::Callee),
    Virtual {
        callee: mir::DispatchCallee,
        slot: u32,
    },
    Interface {
        callee: mir::DispatchCallee,
        interface: mir::InterfaceId,
        slot: u32,
    },
}

impl BoundReferenceTarget {
    pub(super) fn bind(
        self,
        receiver: smir::Expr,
        args: Vec<smir::Expr>,
        return_ty: mir::Type,
    ) -> smir::Call {
        let (target, args) = match self {
            Self::Direct(callee) => (
                mir::CallTarget::Direct(callee),
                std::iter::once(receiver).chain(args).collect(),
            ),
            Self::Virtual { callee, slot } => (
                mir::CallTarget::Virtual {
                    callee,
                    slot,
                    receiver: Box::new(receiver),
                },
                args,
            ),
            Self::Interface {
                callee,
                interface,
                slot,
            } => (
                mir::CallTarget::Interface {
                    callee,
                    interface,
                    slot,
                    receiver: Box::new(receiver),
                },
                args,
            ),
        };
        smir::Call {
            target,
            args,
            return_ty,
        }
    }
}

impl Lowerer {
    pub(super) fn lower_reference_callee(
        &mut self,
        module: &hir::Module,
        callable: hir::CallableTarget,
    ) -> mir::DispatchCallee {
        let callable = match callable {
            hir::CallableTarget::DerivedEquality(target) => {
                return mir::DispatchCallee::External(crate::current::external_equality(
                    &self.external_callables,
                    module.imported_derived_equalities[target].0,
                ));
            }
            hir::CallableTarget::Local(callable) => callable,
            hir::CallableTarget::Imported(callee) => {
                return mir::DispatchCallee::External(
                    self.imported_dependency_callable_map[&callee].scoop_entry(),
                );
            }
        };
        let hir::Callable::Function(function) = callable;
        self.instances.get(function).map_or_else(
            || mir::DispatchCallee::User(self.function_map[&function]),
            mir::DispatchCallee::Monomorphized,
        )
    }

    pub(super) fn bound_reference_target(
        &mut self,
        module: &hir::Module,
        callable: hir::CallableTarget,
    ) -> BoundReferenceTarget {
        let target = self.lower_reference_callee(module, callable);
        let callable = match callable {
            hir::CallableTarget::DerivedEquality(_) => {
                return BoundReferenceTarget::Direct(target.into());
            }
            hir::CallableTarget::Local(callable) => callable,
            hir::CallableTarget::Imported(callee) => {
                return match module.imported_dependency_callables[callee].dispatch() {
                    scoop_hir::ImportedDependencyDispatch::Direct => {
                        BoundReferenceTarget::Direct(target.into())
                    }
                    scoop_hir::ImportedDependencyDispatch::Virtual { slot } => {
                        BoundReferenceTarget::Virtual {
                            callee: target,
                            slot,
                        }
                    }
                    scoop_hir::ImportedDependencyDispatch::Interface { interface, slot } => {
                        let (id, _) = module
                            .interfaces
                            .iter()
                            .find(|(_, declaration)| {
                                declaration.origin.concrete_type_id() == Some(interface)
                            })
                            .expect("a bound dependency reference retains its interface receiver");
                        BoundReferenceTarget::Interface {
                            callee: target,
                            interface: self.interfaces.mir_id(id),
                            slot,
                        }
                    }
                };
            }
        };
        let function = module.callable_function(callable);
        let method = module.functions[function]
            .receiver
            .method()
            .expect("a bound member reference names method metadata");
        match method.dispatch {
            hir::MethodDispatch::Direct | hir::MethodDispatch::FinalOverride(_) => {
                BoundReferenceTarget::Direct(target.into())
            }
            hir::MethodDispatch::Virtual(family) => {
                let hir::TypeKind::Class(class) = module.types[method.owner].kind else {
                    unreachable!("a virtual family belongs to a class method")
                };
                BoundReferenceTarget::Virtual {
                    callee: target,
                    slot: self.method_slots[&self.class_map[&class]][&family],
                }
            }
            hir::MethodDispatch::Interface { interface, slot } => BoundReferenceTarget::Interface {
                callee: target,
                interface: self.interfaces.mir_id(interface),
                slot: slot.into_raw(),
            },
        }
    }
}
