use super::*;

impl Lowerer {
    /// Lower the base-constructor delegation arguments of
    /// `class B(...) : A(args)` (pass 3): arity and types are checked
    /// against A's own constructor properties. The arguments are
    /// lowered in an empty scope — constructor properties are not in
    /// scope there (an M6 simplification: HIR has no body to host the
    /// locals a reference would need).
    pub(crate) fn lower_base_args(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        let Some((_base_ty, args)) = &decl.base_class else {
            return;
        };
        let resolved_base = match self.classes[id].base_class.as_ref() {
            Some((base, _)) => *base,
            None => return, // the clause was rejected in pass 2
        };
        let Type::Class(base_application) = self.types[resolved_base] else {
            unreachable!("resolved class bases are class applications")
        };
        let base_application = self.class_applications[base_application].clone();
        let base_id = base_application.template;
        let base_type_args = base_application.arguments;
        let base_name = self.classes[base_id].name.clone();
        let base_constructor = self.classes[base_id].semantic_constructor().to_vec();
        let props: Vec<(String, TypeId)> = base_constructor
            .iter()
            .map(|field| {
                (
                    field.name.clone(),
                    self.instantiate_ty(field.ty, &base_type_args),
                )
            })
            .collect();
        if args.len() != props.len() {
            let expected = props.len();
            let supplied = args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                decl.span,
                format!(
                    "constructor of class `{base_name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return;
        }
        let mut lowered_args = Vec::with_capacity(args.len());
        let mut ok = true;
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
        self.current_fn_name = format!("<init {base_name}>");
        for (arg, (prop_name, prop_ty)) in args.iter().zip(&props) {
            let mut sink = Vec::new();
            let Some(arg) = self.lower_expr(arg, &mut sink, Some(*prop_ty)) else {
                ok = false;
                break;
            };
            if !sink.is_empty() {
                self.error(
                    arg.span,
                    "`?.` and `?:` are not allowed in base constructor arguments".to_string(),
                );
                ok = false;
                break;
            }
            if !self.is_subtype(arg.ty, *prop_ty) {
                let expected = self.type_name(*prop_ty);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for constructor property `{prop_name}` of class `{base_name}` must be of type {expected}, found {found}"
                    ),
                );
                ok = false;
                break;
            }
            lowered_args.push(self.adapt_to(arg, *prop_ty));
        }
        self.pop_suspension_context();
        self.pop_scope();
        self.constructor_params_in_scope.clear();
        self.type_params_in_scope.clear();
        if ok {
            self.classes[id].base_class = Some((resolved_base, lowered_args));
        }
    }
}
