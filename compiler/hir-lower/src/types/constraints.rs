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
                    hir::TypeParamBounds::Nominal(_) | hir::TypeParamBounds::ImportedNominal(_) => self.error(
                        span,
                        format!(
                            "type parameter `{}` of {target} cannot combine a kind bound with nominal upper bounds",
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
                let bound_name = self.type_name(ty);
                match self.types[ty] {
                    Type::Class(application) => {
                        match &mut params[index].bounds {
                            hir::TypeParamBounds::Unconstrained => {
                                params[index].bounds =
                                    hir::TypeParamBounds::Nominal(hir::NominalBounds {
                                        class: Some(hir::ClassBound { application, span }),
                                        interfaces: Vec::new(),
                                    });
                            }
                            hir::TypeParamBounds::Nominal(bounds) if bounds.class.is_none() => {
                                bounds.class = Some(hir::ClassBound { application, span });
                            }
                            hir::TypeParamBounds::Nominal(_) | hir::TypeParamBounds::ImportedNominal(_) => self.error(
                                span,
                                format!(
                                    "type parameter `{}` of {target} cannot have more than one class upper bound; found `{bound_name}`",
                                    params[index].name
                                ),
                            ),
                            hir::TypeParamBounds::Value { .. }
                            | hir::TypeParamBounds::Ref { .. } => self.error(
                                span,
                                format!(
                                    "type parameter `{}` of {target} cannot combine nominal upper bounds with a kind bound",
                                    params[index].name
                                ),
                            ),
                        }
                    }
                    Type::Interface(application) => {
                        match &mut params[index].bounds {
                            hir::TypeParamBounds::Unconstrained => {
                                params[index].bounds =
                                    hir::TypeParamBounds::Nominal(hir::NominalBounds {
                                        class: None,
                                        interfaces: vec![hir::InterfaceBound {
                                            application,
                                            span,
                                        }],
                                    });
                            }
                            hir::TypeParamBounds::Nominal(bounds)
                                if bounds
                                    .interfaces
                                    .iter()
                                    .all(|existing| existing.application != application) =>
                            {
                                bounds
                                    .interfaces
                                    .push(hir::InterfaceBound { application, span });
                            }
                            hir::TypeParamBounds::Nominal(_) | hir::TypeParamBounds::ImportedNominal(_) => self.error(
                                span,
                                format!(
                                    "duplicate interface upper bound `{bound_name}` for type parameter `{}` of {target}",
                                    params[index].name
                                ),
                            ),
                            hir::TypeParamBounds::Value { .. }
                            | hir::TypeParamBounds::Ref { .. } => self.error(
                                span,
                                format!(
                                    "type parameter `{}` of {target} cannot combine nominal upper bounds with a kind bound",
                                    params[index].name
                                ),
                            ),
                        }
                    }
                    _ => {
                        self.error(
                            reference.span,
                            format!(
                                "upper bound of type parameter `{}` must be a class or interface, found `{bound_name}`",
                                params[index].name
                            ),
                        );
                    }
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
                for bound in parameter.nominal_bounds_in_source_order() {
                    match bound {
                        // Imported bounds were checked with their declaration and
                        // are substituted at each consumer application.
                        hir::NominalBoundRef::ImportedClass(_)
                        | hir::NominalBoundRef::ImportedInterface(_) => continue,
                        hir::NominalBoundRef::Class(bound) => {
                            let application = self.class_applications[bound.application].clone();
                            let target_params =
                                self.classes[application.template].type_params.clone();
                            self.check_type_argument_kinds(
                                &target_params,
                                &application.arguments,
                                bound.span,
                                &format!("upper bound of {kind} `{name}`"),
                            );
                        }
                        hir::NominalBoundRef::Interface(bound) => {
                            let application =
                                self.interface_applications[bound.application].clone();
                            let target_params =
                                self.interfaces[application.template].type_params.clone();
                            self.check_type_argument_kinds(
                                &target_params,
                                &application.arguments,
                                bound.span,
                                &format!("upper bound of {kind} `{name}`"),
                            );
                        }
                    }
                }
            }
        }
        self.type_params_in_scope.clear();
    }
}
