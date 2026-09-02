use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypePosition {
    Covariant,
    Contravariant,
    Invariant,
}

impl TypePosition {
    fn through(self, variance: hir::Variance) -> Self {
        match (self, variance) {
            (Self::Invariant, _) | (_, hir::Variance::Invariant) => Self::Invariant,
            (Self::Covariant, hir::Variance::Out) | (Self::Contravariant, hir::Variance::In) => {
                Self::Covariant
            }
            (Self::Covariant, hir::Variance::In) | (Self::Contravariant, hir::Variance::Out) => {
                Self::Contravariant
            }
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Covariant => "covariant",
            Self::Contravariant => "contravariant",
            Self::Invariant => "invariant",
        }
    }
}

impl Lowerer {
    pub(crate) fn check_interface_inheritance_cycles(
        &mut self,
        pending: &[(hir::InterfaceId, &ast::InterfaceDecl, usize)],
    ) {
        for &(interface, declaration, file) in pending {
            self.current_file = file;
            let mut visiting = Vec::new();
            if self.interface_reaches(interface, interface, &mut visiting) {
                self.error(
                    declaration.span,
                    format!(
                        "interface `{}` directly or indirectly inherits from itself",
                        declaration.name.text
                    ),
                );
            }
        }
    }

    fn interface_reaches(
        &self,
        current: hir::InterfaceId,
        target: hir::InterfaceId,
        visiting: &mut Vec<hir::InterfaceId>,
    ) -> bool {
        if visiting.contains(&current) {
            return false;
        }
        visiting.push(current);
        let reaches = self.interfaces[current].parents.iter().any(|parent| {
            let parent = self.interface_applications[*parent].template;
            parent == target || self.interface_reaches(parent, target, visiting)
        });
        visiting.pop();
        reaches
    }

    /// Validate declaration-site variance against every resolved method
    /// signature. Nested interface applications compose their own variance;
    /// all currently invariant constructors collapse the nested position.
    pub(crate) fn check_interface_variance(&mut self) {
        let interfaces: Vec<hir::InterfaceId> = self.interfaces.iter().map(|(id, _)| id).collect();
        for interface in interfaces {
            if self.interfaces[interface].type_params.is_empty() {
                continue;
            }
            let params = self.interfaces[interface].type_params.clone();
            let methods: Vec<_> = self.interface_methods[&interface]
                .iter()
                .map(|&method| {
                    let signature = &self.signatures[&method];
                    (
                        self.functions[method]
                            .name
                            .rsplit('.')
                            .next()
                            .expect("interface methods are qualified")
                            .to_string(),
                        signature
                            .params
                            .iter()
                            .map(|param| param.ty)
                            .collect::<Vec<_>>(),
                        signature.return_ty,
                        self.functions[method].span,
                    )
                })
                .collect();
            if let Some(&method) = self.interface_methods[&interface].first()
                && let Some(&file) = self.function_files.get(&method)
            {
                self.current_file = file;
            }
            for (method, method_params, return_ty, method_span) in methods {
                for ty in method_params {
                    self.check_variance_position(
                        ty,
                        TypePosition::Contravariant,
                        &params,
                        &method,
                        method_span,
                    );
                }
                self.check_variance_position(
                    return_ty,
                    TypePosition::Covariant,
                    &params,
                    &method,
                    method_span,
                );
            }
        }
    }

    fn check_variance_position(
        &mut self,
        ty: TypeId,
        position: TypePosition,
        params: &[hir::TypeParamDecl],
        method: &str,
        span: ast::Span,
    ) {
        match self.types[ty].clone() {
            Type::Param(id) => {
                // Parameters declared by a generic interface method follow
                // the interface's own prefix and do not participate in the
                // declaration-site variance of that prefix.
                let Some(param) = params.iter().find(|parameter| parameter.id == id) else {
                    return;
                };
                let valid = matches!(param.variance, hir::Variance::Invariant)
                    || matches!(
                        (param.variance, position),
                        (hir::Variance::Out, TypePosition::Covariant)
                            | (hir::Variance::In, TypePosition::Contravariant)
                    );
                if !valid {
                    let declared = match param.variance {
                        hir::Variance::Out => "covariant",
                        hir::Variance::In => "contravariant",
                        hir::Variance::Invariant => unreachable!(),
                    };
                    self.error(
                        span,
                        format!(
                            "{declared} type parameter `{}` occurs in {} position in interface method `{method}`",
                            param.name,
                            position.name()
                        ),
                    );
                }
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let variances: Vec<_> = self.interfaces[application.template]
                    .type_params
                    .iter()
                    .map(|param| param.variance)
                    .collect();
                for (arg, variance) in application.arguments.into_iter().zip(variances) {
                    self.check_variance_position(
                        arg,
                        position.through(variance),
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Struct(application) => {
                let args = self.struct_applications[application].arguments.clone();
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Class(application) => {
                let args = self.class_applications[application].arguments.clone();
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Enum(application) => {
                let args = self.enum_applications[application].arguments.clone();
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Tuple(args) => {
                for arg in args {
                    self.check_variance_position(
                        arg,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
            }
            Type::Ptr(pointee) => {
                self.check_variance_position(pointee, TypePosition::Invariant, params, method, span)
            }
            Type::FunPtr(id) => {
                let function = self.function_types[id].clone();
                for parameter in function.parameter_types {
                    self.check_variance_position(
                        parameter,
                        TypePosition::Invariant,
                        params,
                        method,
                        span,
                    );
                }
                self.check_variance_position(
                    function.return_type,
                    TypePosition::Invariant,
                    params,
                    method,
                    span,
                );
            }
            Type::Function(id) => {
                let function = self.function_types[id].clone();
                for parameter in function.parameter_types {
                    self.check_variance_position(
                        parameter,
                        position.through(hir::Variance::In),
                        params,
                        method,
                        span,
                    );
                }
                self.check_variance_position(
                    function.return_type,
                    position.through(hir::Variance::Out),
                    params,
                    method,
                    span,
                );
            }
            Type::Unit | Type::Int | Type::UInt | Type::Boolean | Type::String | Type::Any => {}
        }
    }
}
