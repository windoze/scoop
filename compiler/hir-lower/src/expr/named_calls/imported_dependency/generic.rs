//! Generic dependency candidates use the same inference session and solver as
//! current declarations. Their successful result retains a provider template.

use super::*;

mod signature;
use crate::call_resolution::arguments::SourceInputId;
use crate::call_resolution::constraints::{
    Constraint, ConstraintOrigin, InferenceSession, TypeTerm,
};
use crate::expr::ResolvedCallTypeArgument;
pub(in crate::expr) use signature::{ImportedGenericTarget, ImportedInferenceSignature};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn probe_imported_generic(
        mut self,
        candidate: ImportedCallableCandidate,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        expected: Option<hir::TypeId>,
        receiver: ImportedCallReceiver,
        argument_map: ImportedArgumentMap,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let declaration = self
            .dependencies
            .as_ref()
            .expect("ordinary calls have a dependency catalog")
            .callable_declaration(candidate.interface().declaration())
            .expect("candidate came from its declaration catalog");
        let template = match ImportedGenericTarget::request(&mut self, declaration) {
            Ok(template) => template,
            Err(error) => {
                self.error(call.span, error);
                return Err(Box::new(self));
            }
        };
        let (signature, template_bindings) = template.signature(&self);
        let signature = &signature;
        let explicit = match self.resolve_call_type_args(call.type_args) {
            Some(arguments) => arguments,
            None => return Err(Box::new(self)),
        };
        let mut session = InferenceSession::new();
        let environment =
            session.add_environment(&signature.owner_parameters, &signature.type_parameters);
        self.add_declaration_bounds(
            &mut session,
            signature
                .owner_parameters
                .iter()
                .chain(&signature.type_parameters),
        );
        if !signature.owner_parameters.is_empty() {
            let ImportedCallReceiver::Member { value, .. } = &receiver else {
                unreachable!("nominal member inference has its exact receiver");
            };
            let hir::PublicDeclarationOwnerV1::Nominal(owner) = candidate.interface().owner()
            else {
                unreachable!("owner parameters belong to a nominal member")
            };
            let owner = self
                .imported_member_owner_type(value.ty(), owner)
                .expect("nominal member inference retains its declared owner application");
            let (_, owner_arguments) = self.types[owner]
                .imported_nominal_application()
                .expect("nominal member inference retains its declared owner application");
            for (variable, argument) in session
                .owner_variables(environment)
                .to_vec()
                .iter()
                .zip(owner_arguments)
            {
                session.push(
                    Constraint::Equal((*variable).into(), TypeTerm::Rigid(*argument)),
                    ConstraintOrigin::Receiver,
                );
            }
        }
        for (index, argument) in explicit.iter().enumerate() {
            if let ResolvedCallTypeArgument::Explicit { ty, .. } = argument {
                let variable = session.callable_variables(environment)[index];
                session.push(
                    Constraint::Equal(variable.into(), TypeTerm::Rigid(*ty)),
                    ConstraintOrigin::ExplicitTypeArgument(index as u32),
                );
            }
        }
        if let Some(expected) = expected {
            session.push(
                Constraint::Subtype(
                    TypeTerm::Type(signature.return_type),
                    TypeTerm::Rigid(expected),
                ),
                ConstraintOrigin::ExpectedResult,
            );
        }
        if let (Some(expected), ImportedCallReceiver::Member { value, .. }) =
            (signature.receiver, &receiver)
        {
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(value.ty()), TypeTerm::Type(expected)),
                ConstraintOrigin::Receiver,
            );
        }
        let mut source_patterns = Vec::new();
        for pattern in argument_map.source_parameters() {
            match self.imported_signature_type_with_bindings(pattern, &template_bindings) {
                Ok(ty) => source_patterns.push(ty),
                Err(error) => {
                    self.error(
                        call.span,
                        format!("cannot resolve dependency parameter type: {error:?}"),
                    );
                    return Err(Box::new(self));
                }
            }
        }
        let mut source_args = Vec::new();
        let mut argument_sinks = Vec::new();
        let mut integer_arguments = Vec::new();
        for (index, pattern) in source_patterns.iter().enumerate() {
            let partial = match self.solve_constraints_partially(&session, environment) {
                Ok(partial) => partial,
                Err(failure) => {
                    self.imported_generic_inference_error(name, call, signature, &failure);
                    return Err(Box::new(self));
                }
            };
            let bindings = signature
                .owner_parameters
                .iter()
                .chain(&signature.type_parameters)
                .zip(partial.owner.into_iter().chain(partial.callable))
                .map(|(p, ty)| {
                    (
                        p.id,
                        ty.unwrap_or_else(|| self.intern_type(hir::Type::Param(p.id))),
                    )
                })
                .collect::<Vec<_>>();
            let hint = self.instantiate_method_ty(*pattern, &bindings);
            let hint = (!self.type_contains_param(hint)).then_some(hint);
            let mut sink = Vec::new();
            let Some(value) = call.arguments.lower(index, &mut self, &mut sink, hint) else {
                return Err(Box::new(self));
            };
            session.push(
                Constraint::Subtype(TypeTerm::Rigid(value.ty), TypeTerm::Type(*pattern)),
                ConstraintOrigin::Argument(SourceInputId::from_index(index)),
            );
            integer_arguments.push(match self.types[value.ty] {
                hir::Type::Integer(kind) => Some(kind),
                _ => None,
            });
            source_args.push(value);
            argument_sinks.push(sink);
        }
        let solution = match self.solve_constraints(&session) {
            Ok(solution) => solution,
            Err(failure) => {
                self.imported_generic_inference_error(name, call, signature, &failure);
                return Err(Box::new(self));
            }
        };
        let solution = solution.arguments_for(&session, environment);
        let bindings = signature
            .owner_parameters
            .iter()
            .chain(&signature.type_parameters)
            .zip(solution.owner.iter().chain(&solution.callable))
            .map(|(p, a)| (p.id, *a))
            .collect::<Vec<_>>();
        let parameter_types = signature
            .parameters
            .iter()
            .skip(usize::from(signature.receiver.is_some()))
            .map(|parameter| self.instantiate_method_ty(parameter.1, &bindings))
            .collect::<Vec<_>>();
        let result_type = self.instantiate_method_ty(signature.return_type, &bindings);
        for (value, pattern) in source_args.iter_mut().zip(&source_patterns) {
            let ty = self.instantiate_method_ty(*pattern, &bindings);
            *value = self.adapt_to(value.clone(), ty);
        }
        let receiver = match receiver {
            ImportedCallReceiver::Member {
                value: ImportedMemberReceiver::Value(value),
                static_type,
            } => {
                let ty = self.instantiate_method_ty(
                    signature
                        .receiver
                        .expect("generic receiver has its declaration type"),
                    &bindings,
                );
                let value = if matches!(
                    candidate.interface().owner(),
                    hir::PublicDeclarationOwnerV1::Nominal(_)
                ) && matches!(self.types[value.ty], hir::Type::Param(_))
                    && matches!(self.types[ty], hir::Type::ImportedInterface(_))
                {
                    value
                } else {
                    self.adapt_to(value, ty)
                };
                ImportedCallReceiver::Member {
                    value: ImportedMemberReceiver::Value(value),
                    static_type,
                }
            }
            receiver => receiver,
        };
        let default_bindings = template_bindings
            .iter()
            .map(|(key, ty)| (key.clone(), self.instantiate_method_ty(*ty, &bindings)))
            .collect();
        let default_plan = match self.prepare_imported_defaults_with_bindings(
            &candidate,
            &argument_map,
            &default_bindings,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                self.error(call.span, error.to_string());
                return Err(Box::new(self));
            }
        };
        let arguments = match template {
            ImportedGenericTarget::Function(id)
                if matches!(
                    self.imported_generic_templates[id].declaration,
                    hir::ImportedCallableTemplateOrigin::Nominal { .. }
                ) =>
            {
                hir::ImportedCallableArguments::Method {
                    owner: self.instantiate_method_ty(
                        signature.receiver.expect("a nominal method has an owner"),
                        &bindings,
                    ),
                    method_arguments: solution.callable,
                }
            }
            _ => hir::ImportedCallableArguments::Function(
                hir::NonEmptyVec::from_vec(solution.callable)
                    .expect("a generic function or constructor has binders"),
            ),
        };
        Ok(ImportedDependencyCallProbe {
            implementation: ImportedCallImplementation::Generic {
                template,
                arguments,
            },
            declaration_file: signature.origin.file as usize,
            declaration_span: signature.span,
            state: Box::new(self),
            candidate,
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            result_type,
            integer_arguments,
            call_span: call.span,
        })
    }

    fn imported_generic_inference_error(
        &mut self,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        signature: &ImportedInferenceSignature,
        failure: &crate::call_resolution::constraints::ConstraintFailure,
    ) {
        let message = self.render_imported_constraint_failure(
            &signature.owner_parameters,
            &signature.type_parameters,
            failure,
        );
        let span = match failure.origin {
            ConstraintOrigin::Argument(input) => call.arguments.span(input.index()),
            ConstraintOrigin::ExplicitTypeArgument(index) => call.type_args[index as usize].span(),
            _ => call.span,
        };
        self.error(
            span,
            format!("dependency function `{}`: {message}", name.text),
        );
    }

    pub(in crate::expr) fn render_imported_constraint_failure(
        &self,
        owner_parameters: &[hir::TypeParamDecl],
        callable_parameters: &[hir::TypeParamDecl],
        failure: &crate::call_resolution::constraints::ConstraintFailure,
    ) -> String {
        use crate::call_resolution::constraints::ConstraintFailureKind as Kind;
        let all_parameters = owner_parameters
            .iter()
            .chain(callable_parameters)
            .cloned()
            .collect::<Vec<_>>();
        let parameter = |variable: crate::call_resolution::constraints::InferenceVariableId| {
            let parameters = match variable {
                crate::call_resolution::constraints::InferenceVariableId::Owner(_) => {
                    owner_parameters
                }
                crate::call_resolution::constraints::InferenceVariableId::Callable(_) => {
                    callable_parameters
                }
            };
            &parameters[variable.group_index()].name
        };
        let type_term = |term: TypeTerm| match term {
            TypeTerm::Variable(variable) => parameter(variable).clone(),
            TypeTerm::Type(ty) | TypeTerm::Rigid(ty) => {
                self.type_name_with_params(ty, &all_parameters)
            }
        };
        match &failure.kind {
            Kind::Kind {
                variable,
                solution,
                required,
            } => format!(
                "type argument `{}` for `{}` must satisfy `{}`",
                self.type_name(*solution),
                parameter(*variable),
                match required {
                    hir::TypeParamKind::Any => "any",
                    hir::TypeParamKind::Value => "value",
                    hir::TypeParamKind::Ref => "ref",
                }
            ),
            Kind::ClassBound {
                variable,
                solution,
                required,
            }
            | Kind::InterfaceBound {
                variable,
                solution,
                required,
            } => format!(
                "type argument `{}` for `{}` must satisfy upper bound `{}`",
                self.type_name(*solution),
                parameter(*variable),
                self.type_name(*required)
            ),
            Kind::UnresolvedTerm(TypeTerm::Variable(variable))
            | Kind::NoUniqueSolution { variable, .. } => format!(
                "cannot infer a unique type argument for `{}`",
                parameter(*variable)
            ),
            Kind::ConflictingExactBounds {
                variable,
                first,
                second,
            } => format!(
                "conflicting types for `{}`: {} and {}",
                parameter(*variable),
                self.type_name(*first),
                self.type_name(*second)
            ),
            Kind::Relation {
                relation,
                left,
                right,
            } => format!(
                "{} {} {}",
                type_term(*left),
                match relation {
                    crate::call_resolution::constraints::RelationKind::Equal => "is not equal to",
                    crate::call_resolution::constraints::RelationKind::Subtype =>
                        "is not a subtype of",
                },
                type_term(*right),
            ),
            Kind::CallableShape(mismatch) => match mismatch {
                crate::call_resolution::constraints::CallableShapeMismatch::ExpectedCallable => {
                    "expected a callable type".into()
                }
                crate::call_resolution::constraints::CallableShapeMismatch::Suspend => {
                    "ordinary and suspend callable shapes differ".into()
                }
                crate::call_resolution::constraints::CallableShapeMismatch::Arity {
                    expected,
                    actual,
                } => format!(
                    "callable shape expects {expected} parameter(s), but the actual type has {actual}"
                ),
            },
            Kind::ForeignVariable(_) => {
                "candidate references an inference variable from another session".into()
            }
            Kind::ForeignTypeParameter(_) => {
                "candidate references a type parameter outside its declaration".into()
            }
            Kind::UnresolvedTerm(term) => format!("cannot resolve type term {}", type_term(*term)),
            Kind::NonConcreteApplication(_) => {
                "candidate result is not a complete concrete type application".into()
            }
        }
    }
}
