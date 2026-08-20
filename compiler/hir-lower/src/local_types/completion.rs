//! Complete environment types once semantic body analysis has found all captures.
use super::*;
use crate::body_transform::{BodyTransform, walk_expression, walk_statement};
use std::collections::HashMap;
mod constructors;
mod regions;

pub(super) struct Start {
    functions: usize,
    constructors: usize,
    defaults: usize,
    local_defaults: usize,
}

impl Start {
    pub(super) fn new(state: &Lowerer) -> Self {
        Self {
            functions: state.functions.len(),
            constructors: state.class_constructors.len(),
            defaults: state.export_default_exprs.len(),
            local_defaults: state.local_default_exprs.len(),
        }
    }
}

impl Lowerer {
    pub(super) fn complete_local_type_captures(&mut self, owner: Owner, start: Start) {
        let Owner::Class(class) = owner else { return };
        let local = &self.local_types[&owner];
        let elements: Vec<_> = local
            .captures
            .iter()
            .map(|key| local.available[key].ty)
            .collect();
        let ty = if elements.is_empty() {
            self.unit
        } else {
            self.intern_type(hir::Type::Tuple(elements))
        };
        let field = self
            .captured_environment_field(class)
            .expect("a local class has an environment");
        let index = self.class_fields[field].definition_index;
        self.classes[class].definition.fields[index].ty = ty;
        let field = self.class_field_identity(field);
        let mut completion = Completion {
            ty,
            field,
            parameters: HashMap::new(),
            bindings: HashMap::new(),
        };
        for id in self.classes[class].constructors.clone() {
            let parameter = self.class_constructors[id]
                .context_parameter
                .as_mut()
                .unwrap();
            let hir::ConstructorContext::CapturedEnvironment(parameter) = parameter else {
                unreachable!()
            };
            parameter.ty = ty;
            completion.parameters.insert(parameter.id, ty);
            completion.bindings.insert(parameter.binding, ty);
        }
        self.complete_constructor_default_context(class, ty);
        completion.declarations(self, start);
    }
}

struct Completion {
    ty: hir::TypeId,
    field: scoop_identity::PersistentFieldId,
    parameters: HashMap<hir::ConstructorParamId, hir::TypeId>,
    bindings: HashMap<hir::BindingId, hir::TypeId>,
}

struct Region<'a> {
    completion: &'a mut Completion,
    locals: &'a mut la_arena::Arena<hir::Local>,
}

impl BodyTransform for Region<'_> {
    fn expression(&mut self, state: &mut Lowerer, expression: &mut hir::Expr) {
        walk_expression(self, state, expression);
        let ty = match &expression.kind {
            hir::ExprKind::ConstructorParam(parameter) => {
                self.completion.parameters.get(parameter).copied()
            }
            hir::ExprKind::Local(local) => self
                .completion
                .bindings
                .get(&self.locals[*local].binding)
                .copied(),
            hir::ExprKind::Capture(binding) => self.completion.bindings.get(binding).copied(),
            hir::ExprKind::FieldAccess {
                field: hir::FieldRef::ClassField { owner, field },
                ..
            } if *field == self.completion.field => {
                let arguments = state
                    .nominal_application(*owner)
                    .expect("class field owner")
                    .arguments;
                Some(state.instantiate_ty(self.completion.ty, &arguments))
            }
            _ => None,
        };
        if let Some(ty) = ty {
            expression.ty = ty;
        }
        match expression.kind {
            hir::ExprKind::Lambda(id) => {
                for capture in &mut state.lambdas[id].captures {
                    capture.ty = capture.source.ty;
                    self.completion.bindings.insert(capture.binding, capture.ty);
                }
            }
            hir::ExprKind::AnonymousFunction(id) => {
                for capture in &mut state.anonymous_functions[id].captures {
                    capture.ty = capture.source.ty;
                    self.completion.bindings.insert(capture.binding, capture.ty);
                }
            }
            hir::ExprKind::CallableReference(id) => {
                for capture in &mut state.callable_references[id].captures {
                    capture.ty = capture.source.ty;
                    self.completion.bindings.insert(capture.binding, capture.ty);
                }
            }
            _ => {}
        }
    }

    fn statement(&mut self, state: &mut Lowerer, statement: &mut hir::Statement) {
        walk_statement(self, state, statement);
        if let hir::StatementKind::LocalFunction(id) = statement.kind {
            let mut local = state.local_functions[id].clone();
            for capture in &mut local.captures {
                self.expression(state, &mut capture.source);
                capture.ty = capture.source.ty;
                self.completion.bindings.insert(capture.binding, capture.ty);
            }
            let function = &mut state.functions[local.source_function()];
            if let hir::FunctionKind::User(body) = &mut function.kind {
                for (capture, parameter) in
                    local.captures.iter().zip(&mut function.signature.params)
                {
                    parameter.ty = capture.ty;
                    body.locals[parameter.local].ty = capture.ty;
                }
            }
            state.local_functions[id] = local;
        }
        if let hir::StatementKind::ValDecl {
            pattern: hir::Pattern::Binding { local },
            init,
        } = &statement.kind
            && self.locals[*local].ty != init.ty
        {
            self.locals[*local].ty = init.ty;
            self.completion
                .bindings
                .insert(self.locals[*local].binding, init.ty);
        }
    }
}
