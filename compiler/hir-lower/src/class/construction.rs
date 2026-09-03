use super::*;

impl Lowerer {
    /// Lower `class Derived(...) : Base(arguments)` with the same source
    /// argument protocol as an ordinary class construction. The resulting
    /// delegation is already positional and carries its complete evaluation
    /// plan, so downstream stages never recover named/default/vararg state.
    pub(crate) fn lower_base_args(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let Some((base_ref, source_args)) = &decl.base_class else {
            return;
        };
        let resolved_base = match self.classes[id].base_class.as_ref() {
            Some((base, _)) => *base,
            None => return,
        };
        let Type::Class(base_application) = self.types[resolved_base] else {
            unreachable!("resolved class bases are class applications")
        };
        let base_application = self.class_applications[base_application].clone();
        let base_id = base_application.template;
        let base_type_args = base_application.arguments;
        let view = self.nominal_constructor_view(
            crate::call_resolution::candidates::NominalConstructorSource::Class(base_id),
        );
        let argument_map =
            match crate::call_resolution::arguments::CandidateArgumentMap::source_nominal(
                &view,
                source_args,
            ) {
                Ok(argument_map) => argument_map,
                Err(failure) => {
                    self.diagnose_nominal_shape_failure(&view, base_ref.span, failure.describe());
                    return;
                }
            };

        self.type_params_in_scope = self.classes[id].type_params.clone();
        self.constructor_params_in_scope = self.classes[id]
            .semantic_constructor()
            .iter()
            .map(|parameter| (parameter.name.clone(), (parameter.parameter, parameter.ty)))
            .collect();
        self.push_scope();
        self.push_suspension_context(SuspensionContext::Forbidden(
            ForbiddenSuspendContext::ConstructorDelegation,
        ));
        self.current_return_ty = self.unit;
        self.current_fn_name = format!("<init {}>", self.classes[id].name);

        let inferred = self.lower_nominal_arguments(crate::expr::NominalArgumentInput {
            view: &view,
            argument_map: &argument_map,
            expressions: source_args,
            explicit_type_args: &base_type_args,
            expected_type_args: None,
            span: base_ref.span,
        });
        let mut statements = Vec::new();
        let arguments = inferred.map(|inferred| {
            debug_assert_eq!(inferred.type_args, base_type_args);
            self.materialize_nominal_arguments(
                crate::argument_materialization::NominalArgumentMaterialization {
                    view: &view,
                    argument_map: &argument_map,
                    type_args: &base_type_args,
                    source_args: inferred.args,
                    argument_sinks: inferred.argument_sinks,
                    call_span: base_ref.span,
                },
                &mut statements,
            )
        });
        let locals = std::mem::take(&mut self.locals);

        self.pop_suspension_context();
        self.pop_scope();
        self.constructor_params_in_scope.clear();
        self.type_params_in_scope.clear();

        if let Some(args) = arguments {
            self.classes[id].base_class = Some((
                resolved_base,
                hir::ConstructorDelegation {
                    locals,
                    statements,
                    args,
                },
            ));
        }
    }
}
