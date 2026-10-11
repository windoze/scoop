use super::*;

impl Lowerer {
    pub(super) fn constructor_enclosing_instance(
        &mut self,
        source: NominalConstructorSource,
        receiver: Option<&hir::Expr>,
        span: ast::Span,
    ) -> Result<Option<hir::Expr>, ()> {
        let NominalConstructorSource::Class(constructor) = source else {
            return Ok(None);
        };
        let class = self.class_constructors[constructor].owner;
        if self.captured_environment_field(class).is_some() {
            return self
                .local_constructor_environment(class, span)
                .map(Some)
                .ok_or(());
        }
        let Some(field) = self.enclosing_class_field(class) else {
            return Ok(None);
        };
        let required = self
            .nominal_application(self.class_field_definition(field).ty)
            .expect("an enclosing field has a class application")
            .template;
        match receiver {
            Some(receiver) => self
                .enclosing_receiver_application(receiver.clone(), required)
                .ok_or_else(|| {
                    self.error(
                        span,
                        "inner constructor requires its declaring outer instance".into(),
                    )
                }),
            None => self.implicit_enclosing_instance(required, span).ok_or(()),
        }
        .map(Some)
    }

    pub(super) fn local_constructor_parameter_count(
        &self,
        source: NominalConstructorSource,
    ) -> usize {
        let owner = match source {
            NominalConstructorSource::Class(id) => {
                crate::Owner::Class(self.class_constructors[id].owner)
            }
            NominalConstructorSource::Struct(id) => {
                crate::Owner::Struct(self.struct_constructors[id].owner)
            }
            NominalConstructorSource::Variant(id) => crate::Owner::Enum(id.enumeration()),
            _ => return 0,
        };
        self.local_type_parameter_count(owner)
    }

    pub(super) fn inner_explicit_arguments(
        &mut self,
        receiver: Option<&hir::Expr>,
        view: &NominalConstructorView,
        call: &NominalConstructorCall<'_>,
    ) -> Option<Vec<ResolvedCallTypeArgument>> {
        let outer = if let Some(receiver) =
            receiver.filter(|receiver| matches!(self.types[receiver.ty], hir::Type::Class(_)))
        {
            self.nominal_application(receiver.ty)
                .expect("an enclosing receiver has its class application")
                .arguments
        } else {
            let count = self.local_constructor_parameter_count(view.target);
            if count == 0 {
                return Some(call.explicit_type_args.to_vec());
            }
            view.signature.owner_parameters[..count]
                .iter()
                .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
                .collect()
        };
        let count = view.signature.owner_parameters.len() - outer.len();
        if !call.explicit_type_args.is_empty() && call.explicit_type_args.len() != count {
            self.diagnose_nominal_shape_failure(
                view,
                call.span,
                format!(
                    "expects {count} explicit type argument(s), but {} were supplied",
                    call.explicit_type_args.len(),
                ),
            );
            return None;
        }
        let mut explicit = outer
            .into_iter()
            .map(|ty| ResolvedCallTypeArgument::Explicit {
                ty,
                span: call.span,
            })
            .collect::<Vec<_>>();
        if call.explicit_type_args.is_empty() {
            explicit
                .extend((0..count).map(|_| ResolvedCallTypeArgument::Infer { span: call.span }));
        } else {
            explicit.extend_from_slice(call.explicit_type_args);
        }
        Some(explicit)
    }
}
