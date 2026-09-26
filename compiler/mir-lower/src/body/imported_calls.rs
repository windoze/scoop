use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_imported_call(
        &mut self,
        callee: hir::ImportedDependencyCallableUseId,
        args: &[hir::Expr],
        result_type: hir::TypeId,
    ) -> smir::Expr {
        let dispatch = self.module.imported_dependency_callables[callee].dispatch();
        let (callee, role) = self.imported_dependency_callable_map[&callee];
        let callee = mir::Callee::External(callee);
        let return_ty = self.lower_type(result_type);
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
