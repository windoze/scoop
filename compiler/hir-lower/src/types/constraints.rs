use super::*;

impl Lowerer {
    /// Resolve the complete constraint set for one declaration after all
    /// nominal names and arities are known. `params` may begin with an owner
    /// prefix (generic methods/local functions); only names declared in
    /// `declarations` may be constrained by this declaration's `where` clause.
    pub(crate) fn resolve_type_parameter_constraints(
        &mut self,
        mut params: Vec<hir::TypeParamDecl>,
        owner_count: usize,
        declarations: &[ast::TypeParamDecl],
        where_clause: Option<&ast::WhereClause>,
        target: &str,
    ) -> Vec<hir::TypeParamDecl> {
        self.type_params_in_scope = params.clone();
        let own_indices: std::collections::HashMap<_, _> = params
            .iter()
            .enumerate()
            .skip(owner_count)
            .map(|(index, parameter)| (parameter.name.clone(), index))
            .collect();

        let mut seen_inline = std::collections::HashSet::new();
        for declaration in declarations {
            if !seen_inline.insert(declaration.name.text.as_str()) {
                continue;
            }
            let Some(bound) = declaration.inline_bound.as_ref() else {
                continue;
            };
            let Some(&index) = own_indices.get(&declaration.name.text) else {
                continue;
            };
            self.apply_type_parameter_constraint(
                &mut params,
                index,
                bound,
                declaration.span,
                target,
            );
        }

        if let Some(clause) = where_clause {
            for constraint in &clause.constraints {
                let Some(&index) = own_indices.get(&constraint.parameter.text) else {
                    self.error(
                        constraint.parameter.span,
                        format!(
                            "unknown type parameter `{}` in where clause of {target}",
                            constraint.parameter.text
                        ),
                    );
                    continue;
                };
                self.apply_type_parameter_constraint(
                    &mut params,
                    index,
                    &constraint.bound,
                    constraint.span,
                    target,
                );
            }
        }

        self.type_params_in_scope.clear();
        params
    }

    fn apply_type_parameter_constraint(
        &mut self,
        params: &mut [hir::TypeParamDecl],
        index: usize,
        bound: &ast::TypeBound,
        span: ast::Span,
        target: &str,
    ) {
        match bound {
            ast::TypeBound::Kind(kind) => {
                let next = match kind {
                    ast::TypeParamKindBound::Value => hir::TypeParamBounds::Value { span },
                    ast::TypeParamKindBound::Ref => hir::TypeParamBounds::Ref { span },
                };
                match params[index].bounds {
                    hir::TypeParamBounds::Unconstrained => params[index].bounds = next,
                    hir::TypeParamBounds::Interfaces(_) => self.error(
                        span,
                        format!(
                            "type parameter `{}` of {target} cannot combine a kind bound with interface upper bounds",
                            params[index].name
                        ),
                    ),
                    hir::TypeParamBounds::Value { .. }
                    | hir::TypeParamBounds::Ref { .. } => self.error(
                        span,
                        format!(
                            "duplicate kind bound for type parameter `{}` of {target}",
                            params[index].name
                        ),
                    ),
                }
            }
            ast::TypeBound::Upper(reference) => {
                // Keep the complete parameter namespace visible while the
                // upper application (including F-bound arguments) resolves.
                self.type_params_in_scope = params.to_vec();
                let Some(ty) = self.resolve_type_ref(reference) else {
                    return;
                };
                let Type::Interface(application) = self.types[ty] else {
                    let found = self.type_name(ty);
                    self.error(
                        reference.span,
                        format!(
                            "upper bound of type parameter `{}` must be an interface, found `{found}`",
                            params[index].name
                        ),
                    );
                    return;
                };
                match &mut params[index].bounds {
                    hir::TypeParamBounds::Unconstrained => {
                        params[index].bounds =
                            hir::TypeParamBounds::Interfaces(vec![hir::InterfaceBound {
                                application,
                                span,
                            }]);
                    }
                    hir::TypeParamBounds::Interfaces(bounds) => {
                        if bounds
                            .iter()
                            .any(|existing| existing.application == application)
                        {
                            self.error(
                                span,
                                format!(
                                    "duplicate interface upper bound `{}` for type parameter `{}` of {target}",
                                    self.type_name(ty), params[index].name
                                ),
                            );
                        } else {
                            bounds.push(hir::InterfaceBound { application, span });
                        }
                    }
                    hir::TypeParamBounds::Value { .. }
                    | hir::TypeParamBounds::Ref { .. } => self.error(
                        span,
                        format!(
                            "type parameter `{}` of {target} cannot combine interface upper bounds with a kind bound",
                            params[index].name
                        ),
                    ),
                }
            }
        }
        self.type_params_in_scope = params.to_vec();
    }

    /// Validate dependencies between nominal bound declarations only after all
    /// of them have complete constraint sets. This is order-independent and
    /// permits legal F-bound cycles.
    pub(crate) fn validate_nominal_type_parameter_constraints(&mut self) {
        let declarations: Vec<_> = self
            .structs
            .iter()
            .map(|(id, declaration)| {
                (
                    self.struct_files[&id],
                    "struct",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            })
            .chain(self.enums.iter().map(|(id, declaration)| {
                (
                    self.enum_files[&id],
                    "enum",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            }))
            .chain(self.classes.iter().map(|(id, declaration)| {
                (
                    self.class_files[&id],
                    "class",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            }))
            .chain(self.interfaces.iter().map(|(id, declaration)| {
                (
                    self.interface_files[&id],
                    "interface",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            }))
            .collect();
        for (file, kind, name, params) in declarations {
            self.current_file = file;
            self.type_params_in_scope = params.clone();
            for parameter in &params {
                for bound in parameter.interface_bounds() {
                    let application = self.interface_applications[bound.application].clone();
                    let target_params = self.interfaces[application.template].type_params.clone();
                    self.check_type_argument_kinds(
                        &target_params,
                        &application.arguments,
                        bound.span,
                        &format!("upper bound of {kind} `{name}`"),
                    );
                }
            }
        }
        self.type_params_in_scope.clear();
    }
}
