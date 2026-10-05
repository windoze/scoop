use super::*;

pub(super) struct LambdaParameters {
    pub(super) abi: Vec<hir::Param>,
    pub(super) types: Vec<TypeId>,
    pub(super) prefix: Vec<hir::Statement>,
}

impl Lowerer {
    pub(super) fn lower_lambda_parameters(
        &mut self,
        source_parameters: &[Option<&ast::LambdaParam>],
        expected_signature: Option<&hir::FunctionType>,
        span: Span,
    ) -> Option<LambdaParameters> {
        let mut abi_params = Vec::with_capacity(source_parameters.len());
        let mut parameter_types = Vec::with_capacity(source_parameters.len());
        let mut prefix = Vec::new();
        for (index, parameter) in source_parameters.iter().enumerate() {
            let expected_ty = expected_signature.map(|signature| signature.parameter_types[index]);
            let explicit_ty = match parameter.and_then(|parameter| parameter.ty.as_ref()) {
                Some(ty) => Some(self.resolve_type_ref(ty)?),
                None => None,
            };
            let parameter_ty = match (explicit_ty, expected_ty) {
                (Some(explicit), Some(expected)) => {
                    if !self.is_subtype(expected, explicit) {
                        let found = self.type_name(explicit);
                        let expected = self.type_name(expected);
                        let at = parameter
                            .expect("an explicit type belongs to a parameter")
                            .span;
                        self.error(
                            at,
                            format!(
                                "lambda parameter type is {found}, but the expected type is {expected}"
                            ),
                        );
                        return None;
                    }
                    explicit
                }
                (Some(explicit), None) => explicit,
                (None, Some(expected)) => expected,
                (None, None) => {
                    let at = parameter.map_or(span, |parameter| parameter.span);
                    self.error(
                        at,
                        "lambda parameter requires a type when there is no expected function type"
                            .to_string(),
                    );
                    return None;
                }
            };
            parameter_types.push(parameter_ty);
            let target = parameter.map(|parameter| &parameter.target);
            let binding_name = match target {
                Some(ast::Pattern::Binding(name)) => Some(name.clone()),
                None => Some(ast::Ident {
                    text: "it".to_string(),
                    span,
                }),
                _ => None,
            };
            if let Some(name) = binding_name {
                if self.scopes.is_declared_here(&name.text) {
                    self.error(
                        name.span,
                        format!("`{}` is already declared in this scope", name.text),
                    );
                    return None;
                }
                let local =
                    self.alloc_parameter_local(name.text.clone(), parameter_ty, index, name.span);
                self.scopes.declare(name.text.clone(), local);
                abi_params.push(hir::Param {
                    name: name.text,
                    ty: parameter_ty,
                    local,
                });
            } else {
                let local = self.alloc_parameter_local(
                    format!("$arg.{index}"),
                    parameter_ty,
                    index,
                    parameter
                        .expect("a destructured lambda parameter is explicit")
                        .span,
                );
                let plan = self.lower_irrefutable_binding_from_subject(
                    target.expect("non-binding source parameter has a pattern"),
                    crate::patterns::BindingSubject {
                        local,
                        ty: parameter_ty,
                    },
                    false,
                )?;
                prefix.extend(plan);
                abi_params.push(hir::Param {
                    name: format!("$arg.{index}"),
                    ty: parameter_ty,
                    local,
                });
            }
        }
        Some(LambdaParameters {
            abi: abi_params,
            types: parameter_types,
            prefix,
        })
    }
}
