use super::*;

mod bounds;
mod companions;
mod negative;

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
        function_conditions: bool,
        target: &str,
    ) -> Vec<hir::TypeParamDecl> {
        let outer_checks = self.deferred_type_bounds.replace(Vec::new());
        self.type_params_in_scope = params.clone();
        let own_indices: std::collections::HashMap<_, _> = params
            .iter()
            .enumerate()
            .skip(owner_count)
            .map(|(index, parameter)| (parameter.name.clone(), index))
            .collect();

        self.declare_companion_conditions(
            &mut params,
            &own_indices,
            where_clause,
            function_conditions,
        );

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
                (index, false),
                bound,
                declaration.span,
                function_conditions,
                target,
            );
        }

        if let Some(clause) = where_clause {
            for constraint in &clause.constraints {
                let Some(&index) = own_indices.get(&constraint.subject.parameter().text) else {
                    self.error(
                        constraint.subject.parameter().span,
                        format!(
                            "unknown type parameter `{}` in where clause of {target}",
                            constraint.subject.parameter().text
                        ),
                    );
                    continue;
                };
                self.apply_type_parameter_constraint(
                    &mut params,
                    (
                        index,
                        matches!(constraint.subject, ast::TypeConstraintSubject::Companion(_)),
                    ),
                    &constraint.bound,
                    constraint.span,
                    function_conditions,
                    target,
                );
            }
        }

        self.type_params_in_scope = params.clone();
        self.check_negative_bound_contradictions(&params[owner_count..]);
        self.complete_parameter_bound_checks(outer_checks, &params);
        self.type_params_in_scope.clear();
        params
    }

    /// Validate dependencies between nominal bound declarations only after all
    /// of them have complete constraint sets. This is order-independent and
    /// permits legal F-bound cycles.
    pub(crate) fn validate_nominal_type_parameter_constraints(&mut self) {
        let object_backings: std::collections::HashSet<_> = self
            .objects
            .values()
            .map(|object| object.backing_class)
            .collect();
        let owners = self
            .structs
            .iter()
            .map(|(id, _)| crate::Owner::Struct(id))
            .chain(self.enums.iter().map(|(id, _)| crate::Owner::Enum(id)))
            .chain(
                self.classes
                    .iter()
                    .filter(|(id, _)| !object_backings.contains(id))
                    .map(|(id, _)| crate::Owner::Class(id)),
            )
            .chain(
                self.interfaces
                    .iter()
                    .map(|(id, _)| crate::Owner::Interface(id)),
            )
            .collect::<Vec<_>>();
        self.validate_owner_type_parameter_constraints(&owners);
    }

    pub(crate) fn validate_owner_type_parameter_constraints(&mut self, owners: &[crate::Owner]) {
        for &owner in owners {
            self.current_file = match owner {
                crate::Owner::Class(id) => self.class_files[&id],
                crate::Owner::Struct(id) => self.struct_files[&id],
                crate::Owner::Enum(id) => self.enum_files[&id],
                crate::Owner::Interface(id) => self.interface_files[&id],
                crate::Owner::Object(id) => self.object_files[&id],
            };
            let name = owner.describe(self);
            let params = self.owner_type_params(owner);
            self.type_params_in_scope = params.clone();
            for parameter in &params {
                for bound in parameter.nominal_bounds_in_source_order() {
                    let (target_params, arguments) = match self.types[bound.ty()] {
                        Type::Class(application) => {
                            let application = self.class_applications[application].clone();
                            let Some(class) = self.source_class_id(application.template) else {
                                continue;
                            };
                            (
                                self.classes[class].type_params.clone(),
                                application.arguments,
                            )
                        }
                        Type::Interface(application) => {
                            let application = self.interface_applications[application].clone();
                            let Some(source_id) = self.source_interface_id(application.template)
                            else {
                                continue;
                            };
                            (
                                self.interfaces[source_id].type_params.clone(),
                                application.arguments,
                            )
                        }
                        _ => unreachable!("nominal bounds retain a class or interface type"),
                    };
                    self.check_type_argument_kinds(
                        &target_params,
                        &arguments,
                        bound.span(),
                        &format!("upper bound of {name}"),
                    );
                }
            }
        }
        self.type_params_in_scope.clear();
    }
}
