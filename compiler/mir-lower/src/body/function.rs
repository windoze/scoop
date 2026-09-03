use super::*;

mod adapters;

impl BodyLowerer<'_> {
    pub(crate) fn lower_function(
        mut self,
        function: &hir::Function,
        body: &hir::Body,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        for (hir_id, local) in body.locals.iter() {
            let ty = self.lower_type(local.ty);
            let mir_id = self.locals.alloc(mir::Local {
                name: local.name.clone(),
                ty,
                mutable: local.mutable,
            });
            self.local_map.insert(hir_id, mir_id);
        }
        let params = function
            .params
            .iter()
            .map(|param| mir::Param {
                name: param.name.clone(),
                ty: self.lower_type(param.ty),
                local: self.local_map[&param.local],
            })
            .collect();
        if self.current_closure.is_some() {
            self.current_closure_local = function
                .params
                .first()
                .map(|param| self.local_map[&param.local]);
        }
        let return_ty = self.lower_type(function.return_ty);
        let statements = if is_abstract_bodiless(function) {
            // An abstract method (hir-lower materializes it bodiless):
            // every override replaces its vtable slot and the class
            // cannot be instantiated, so the slot is never reached;
            // the emitted function traps like a pure-virtual stub.
            let message =
                self.trap_message(format!("call to abstract method `{}`", fn_name(function)));
            vec![smir::Statement {
                kind: smir::StatementKind::Expr(smir::Expr::new(
                    mir::Type::Unit,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::Runtime(mir::RuntimeFn::Trap),
                        },
                        args: vec![smir::Expr::new(
                            mir::Type::String,
                            smir::ExprKind::StringConst(message),
                        )],
                        return_ty: mir::Type::Unit,
                    }),
                )),
                span: function.span,
            }]
        } else {
            self.lower_statements(&body.statements)
        };
        (
            params,
            return_ty,
            smir::Body {
                locals: self.locals,
                statements,
            },
        )
    }

    pub(crate) fn lower_type(&mut self, ty: hir::TypeId) -> mir::Type {
        Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        }
        .lower(ty, self.enums, self.structs, self.interfaces, self.shell)
    }

    pub(crate) fn lower_function_type_id(
        &mut self,
        function_type: hir::FunctionTypeId,
    ) -> mir::FunctionTypeId {
        let ty = self.module.function_types[function_type].canonical_type;
        let mir::Type::Function(function_type) = self.lower_type(ty) else {
            unreachable!("lowering a function type preserves its category")
        };
        function_type
    }

    /// A fresh hidden local (`$<prefix>.<n>`), compiler-generated.
    pub(super) fn new_hidden(
        &mut self,
        prefix: &str,
        ty: mir::Type,
        mutable: bool,
    ) -> mir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable,
        })
    }

    /// Emit the queued prelude statements (the trap tests of `!!`)
    /// before the statement they belong to.
    pub(super) fn drain_prelude(&mut self, span: Span, out: &mut Vec<smir::Statement>) {
        out.extend(
            self.prelude
                .drain(..)
                .map(|kind| smir::Statement { kind, span }),
        );
    }
}
