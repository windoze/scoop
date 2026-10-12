use super::*;

impl Lowerer {
    pub(crate) fn local_constructor_capture_binding(
        &self,
        class: hir::ClassId,
    ) -> Option<hir::BindingId> {
        self.local_type_capture_scope
            .as_ref()
            .filter(|scope| scope.owner == Owner::Class(class))
            .and_then(|scope| scope.environment.as_ref().map(|(_, binding)| *binding))
    }

    pub(crate) fn captured_environment_field(
        &self,
        class: hir::ClassId,
    ) -> Option<hir::ClassFieldId> {
        self.classes[class].fields.iter().copied().find(|field| {
            self.class_fields[*field].source == hir::ClassFieldSource::CapturedEnvironment
        })
    }

    pub(crate) fn local_type_receivers(&self, owner: Owner) -> Vec<crate::ReceiverScope> {
        self.local_type_root(owner)
            .map_or_else(Vec::new, |root| self.local_types[&root].receivers.clone())
    }

    pub(crate) fn enter_local_type_captures(
        &mut self,
        owner: Owner,
        environment: Option<hir::Expr>,
    ) -> Option<CaptureScope> {
        if let Some(root) = self.local_type_root(owner) {
            self.smart_casts = self.local_types[&root].facts.clone();
        }
        let scope = self.local_type_root(owner).map(|owner| CaptureScope {
            owner,
            environment: environment.map(|value| {
                let binding = match value.kind {
                    hir::ExprKind::Local(id) => self.locals[id].binding,
                    hir::ExprKind::ConstructorParam(id) => self.constructor_parameter_bindings[&id],
                    _ => unreachable!("a captured environment is a saved input"),
                };
                (value, binding)
            }),
        });
        std::mem::replace(&mut self.local_type_capture_scope, scope)
    }

    pub(crate) fn local_type_instance_environment(
        &mut self,
        mut instance: hir::Expr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        loop {
            let hir::Type::Class(application) = self.types[instance.ty] else {
                return None;
            };
            let owner = self.class_applications[application].template;
            if let Some(Owner::Class(class)) = self.nominal_owners.get(&owner).copied()
                && let Some(field) = self.captured_environment_field(class)
            {
                let ty = self.class_field_definition(field).ty;
                let arguments = self.class_applications[application].arguments.clone();
                let ty = self.instantiate_ty(ty, &arguments);
                let span = instance.span;
                let value = hir::Expr {
                    kind: hir::ExprKind::FieldAccess {
                        receiver: Box::new(instance),
                        field: self.class_field_reference(application, field),
                    },
                    ty,
                    span,
                    origin: self.expression_origin(span),
                };
                return Some(self.materialize_temporary("$captures".into(), value, span, sink));
            }
            instance = self.enclosing_instance_expression(instance)?;
        }
    }
}
