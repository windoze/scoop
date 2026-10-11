use super::*;

impl Completion {
    pub(super) fn region(
        &mut self,
        state: &mut Lowerer,
        locals: &mut la_arena::Arena<hir::Local>,
        statements: &mut [hir::Statement],
        values: &mut [hir::Expr],
    ) {
        for (_, local) in locals.iter_mut() {
            if let Some(&ty) = self.bindings.get(&local.binding) {
                local.ty = ty;
            }
        }
        let mut region = Region {
            completion: self,
            locals,
        };
        region.statements(state, statements);
        for value in values {
            region.expression(state, value);
        }
    }

    pub(super) fn body(&mut self, state: &mut Lowerer, body: &mut hir::Body) {
        self.region(state, &mut body.locals, &mut body.statements, &mut []);
    }

    fn default(&mut self, state: &mut Lowerer, default: &mut hir::ExportDefaultExpr) {
        if let hir::LexicalDefinitionRoot::ClassConstructor(id) = default.definition_root
            && let Some(parameter) = &state.class_constructors[id].context_parameter
            && let Some(&ty) = self.parameters.get(&parameter.id)
            && let Some(receiver) = &mut default.expression.receiver
        {
            receiver.ty = ty;
            self.bindings
                .insert(default.expression.body.locals[receiver.local].binding, ty);
        }
        let expression = &mut default.expression;
        self.region(
            state,
            &mut expression.body.locals,
            &mut expression.body.statements,
            std::slice::from_mut(&mut expression.value),
        );
    }

    pub(super) fn declarations(&mut self, state: &mut Lowerer, start: Start) {
        let constructors = state
            .class_constructors
            .iter()
            .skip(start.constructors)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in constructors {
            let mut value = state.class_constructors[id].clone();
            self.constructor(state, &mut value.kind);
            state.class_constructors[id] = value;
        }
        let defaults = state
            .export_default_exprs
            .iter()
            .skip(start.defaults)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in defaults {
            let mut value = state.export_default_exprs[id].clone();
            self.default(state, &mut value);
            state.export_default_exprs[id] = value;
        }
        let defaults = state
            .local_default_exprs
            .iter()
            .skip(start.local_defaults)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in defaults {
            let mut value = state.local_default_exprs[id].clone();
            self.default(state, &mut value.body);
            state.local_default_exprs[id] = value;
        }
        let functions = state
            .functions
            .iter()
            .skip(start.functions)
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in functions {
            let mut function = state.functions[id].clone();
            if let hir::FunctionKind::User(body) = &mut function.kind {
                self.body(state, body);
                for param in &mut function.signature.params {
                    param.ty = body.locals[param.local].ty;
                }
            }
            state.functions[id] = function;
        }
    }
}
