use super::*;

mod native;

impl BodyLowerer<'_> {
    pub(super) fn lower_imported_equality(
        &mut self,
        target: hir::ImportedDerivedEqualityUseId,
        args: &[hir::Expr],
        result: hir::TypeId,
    ) -> smir::Expr {
        let target = self.module.imported_derived_equalities[target].0;
        let callee = crate::current::external_equality(self.external_callables, target);
        let return_ty = self.lower_type(result);
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::External(callee),
                },
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                return_ty,
            }),
        )
    }

    pub(super) fn lower_imported_call(
        &mut self,
        callee: hir::ImportedDependencyCallableUseId,
        args: &[hir::Expr],
        result_type: hir::TypeId,
    ) -> smir::Expr {
        let source = self.module.imported_dependency_callables[callee];
        self.contains_suspend_call |= source.effect() == scoop_identity::Effect::Suspend;
        let dispatch = source.dispatch();
        let target = self.imported_dependency_callable_map[&callee].clone();
        let role = target.lowering_role;
        let return_ty = self.lower_type(result_type);
        let callee = if matches!(self.current_owner, mir::LocalValueOwner::ReleaseHook(_)) {
            target
                .native_c()
                .and_then(|native| self.release_native_target(native, args, return_ty.clone()))
                .map(mir::Callee::Extern)
                .unwrap_or_else(|| mir::Callee::External(target.scoop_entry()))
        } else {
            mir::Callee::External(target.scoop_entry())
        };
        if let mir::MirCallableLoweringRoleV1::ClassInitializer { .. } = role {
            // The source expression returns the allocated class, while the
            // physical initializer call returns Unit.
            self.lower_type(self.module.unit);
            let mir::Type::Class(class_id) = return_ty else {
                unreachable!("a class initializer has a class semantic result")
            };
            return smir::Expr::new(
                return_ty,
                smir::ExprKind::ClassNew {
                    class_id,
                    publish_release: {
                        let hir::TypeKind::Class(class) = &self.module.types[result_type].kind
                        else {
                            unreachable!("an imported initializer constructs its exact class")
                        };
                        !matches!(
                            self.module.classes[*class].release_policy,
                            hir::ReleasePolicy::None
                        )
                    },
                    initializer: callee,
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                },
            );
        }
        let kind = match dispatch {
            scoop_hir::ImportedDependencyDispatch::Direct => mir::CallKind::Direct,
            scoop_hir::ImportedDependencyDispatch::Virtual { slot } => {
                mir::CallKind::Virtual { slot }
            }
            scoop_hir::ImportedDependencyDispatch::Interface { interface, slot } => {
                let ty = self
                    .module
                    .interfaces
                    .iter()
                    .find(|(_, declaration)| {
                        declaration.origin.concrete_type_id() == Some(interface)
                    })
                    .expect("a selected interface call retains its receiver type")
                    .1
                    .canonical_type;
                let mir::Type::Interface(interface) = self.lower_type(ty) else {
                    unreachable!("an interface receiver retains its type kind")
                };
                mir::CallKind::Interface { interface, slot }
            }
        };
        smir::Expr::new(
            return_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget { kind, callee },
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                return_ty,
            }),
        )
    }
}
