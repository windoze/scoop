use super::*;

mod adapters;

impl BodyLowerer<'_> {
    pub(crate) fn allocate_function_locals(&mut self, function: hir::FunctionId, body: &hir::Body) {
        let identities = body
            .locals
            .iter()
            .map(|(local, _)| {
                self.module
                    .local_value_identities
                    .function_local(function, local)
                    .clone()
            })
            .collect();
        self.allocate_source_locals(&body.locals, identities);
    }

    pub(crate) fn allocate_class_locals(
        &mut self,
        constructor: hir::ClassConstructorId,
        body: &hir::Body,
    ) {
        let identities = body
            .locals
            .iter()
            .map(|(local, _)| {
                self.module
                    .local_value_identities
                    .class_local(constructor, local)
                    .clone()
            })
            .collect();
        self.allocate_source_locals(&body.locals, identities);
    }

    pub(crate) fn allocate_struct_argument_locals(
        &mut self,
        constructor: hir::StructConstructorId,
        body: &hir::ConstructorArguments,
    ) {
        let identities = body
            .locals
            .iter()
            .map(|(local, _)| {
                self.module
                    .local_value_identities
                    .struct_argument_local(constructor, local)
                    .expect("secondary constructor argument locals have persistent identities")
                    .clone()
            })
            .collect();
        self.allocate_source_locals(&body.locals, identities);
    }

    pub(crate) fn allocate_struct_body_locals(
        &mut self,
        constructor: hir::StructConstructorId,
        body: &hir::Body,
    ) {
        let identities = body
            .locals
            .iter()
            .map(|(local, _)| {
                self.module
                    .local_value_identities
                    .struct_body_local(constructor, local)
                    .expect("secondary constructor body locals have persistent identities")
                    .clone()
            })
            .collect();
        self.allocate_source_locals(&body.locals, identities);
    }

    fn allocate_source_locals(
        &mut self,
        locals: &Arena<hir::Local>,
        identities: Vec<hir::LocalValueIdentityRecord>,
    ) {
        assert_eq!(
            locals.len(),
            identities.len(),
            "the LocalConcrete local-value relation is total"
        );
        for ((hir_id, local), identity) in locals.iter().zip(identities) {
            let ty = self.lower_type(local.ty);
            let mir_id = self.locals.alloc(mir::Local {
                name: local.name.clone(),
                ty,
                mutable: local.mutable,
            });
            self.local_map.insert(hir_id, mir_id);
            self.local_values
                .record(self.current_function, mir_id, &identity);
        }
    }

    pub(crate) fn lower_function(
        mut self,
        function_id: hir::FunctionId,
        function: &hir::Function,
        body: &hir::Body,
    ) -> (Vec<mir::Param>, mir::Type, smir::Body) {
        self.allocate_function_locals(function_id, body);
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
        assert!(
            self.active_loops.is_empty(),
            "structured loop remapping is balanced at a callable boundary"
        );
        let coroutine_eh = self.coroutine_eh_mode();
        (
            params,
            return_ty,
            smir::Body {
                locals: self.locals,
                statements,
                coroutine_eh,
            },
        )
    }

    pub(crate) fn coroutine_eh_mode(&self) -> Option<smir::CoroutineEhMode> {
        self.contains_suspend_call.then(|| smir::CoroutineEhMode {
            throwable: mir::Type::Class(
                self.class_map[&self.core_protocols.defined().exceptions.throwable.class()],
            ),
        })
    }

    pub(crate) fn lower_type(&mut self, ty: hir::TypeId) -> mir::Type {
        let lowered = Types {
            module: self.module,
            struct_map: self.struct_map,
            class_map: self.class_map,
        }
        .lower(
            ty,
            self.source_exact_types,
            self.enums,
            self.structs,
            self.interfaces,
            self.shell,
        );
        let source = self
            .source_exact_types
            .get(&lowered)
            .expect("every lowered HIR type has a persistent MIR relation");
        assert_eq!(
            source.identity_record().id(),
            self.module.exact_type_identities[ty].id(),
            "type lowering preserves the exact HIR identity"
        );
        lowered
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
        let local = self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable,
        });
        self.local_values.record_generated_local(
            self.current_function,
            local,
            self.current_materialization,
            hir::StructuralDefinitionSiteRole::SyntheticValue,
            hir::SyntheticLocalRole::Temporary,
        );
        local
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
