use super::*;

impl Lowerer {
    pub(super) fn resolve_class_secondary_edge(
        &mut self,
        owner: ClassId,
        primary: Option<hir::ClassConstructorId>,
        constructor: hir::ClassConstructorId,
        source: &ast::SecondaryConstructorDecl,
    ) {
        if primary.is_some() {
            let Some(ast::ConstructorDelegation::This {
                arguments, span, ..
            }) = source.delegation.as_ref()
            else {
                let message = if matches!(
                    source.delegation,
                    Some(ast::ConstructorDelegation::Super { .. })
                ) {
                    "a class with a primary constructor requires every secondary constructor to delegate with `this(...)`, not `super(...)`"
                } else {
                    "a class with a primary constructor requires every secondary constructor to declare `this(...)` delegation"
                };
                self.error(source.span, message.into());
                return;
            };
            let Some((target, arguments)) = self.lower_class_this_edge(
                owner,
                constructor,
                arguments,
                *span,
                "secondary `this` delegation",
            ) else {
                return;
            };
            self.set_class_secondary_delegation(
                constructor,
                hir::ClassSecondaryDelegation::This { target, arguments },
            );
            return;
        }

        match source.delegation.as_ref() {
            Some(ast::ConstructorDelegation::This {
                arguments, span, ..
            }) => {
                let Some((target, arguments)) = self.lower_class_this_edge(
                    owner,
                    constructor,
                    arguments,
                    *span,
                    "secondary `this` delegation",
                ) else {
                    return;
                };
                self.set_class_secondary_delegation(
                    constructor,
                    hir::ClassSecondaryDelegation::This { target, arguments },
                );
            }
            Some(ast::ConstructorDelegation::Super {
                arguments, span, ..
            }) => {
                let Some(base_ty) = self.classes[owner].base_class else {
                    self.error(*span, "`super(...)` requires a direct base class".into());
                    return;
                };
                let Some(base) = self.lower_base_initialization(
                    constructor,
                    base_ty,
                    arguments,
                    *span,
                    "secondary `super` delegation",
                ) else {
                    return;
                };
                self.set_class_secondary_delegation(
                    constructor,
                    hir::ClassSecondaryDelegation::Terminal {
                        base,
                        common_initialization: Vec::new(),
                    },
                );
            }
            None => {
                let base = match self.classes[owner].base_class {
                    Some(base_ty) => {
                        let Some(base) = self.lower_base_initialization(
                            constructor,
                            base_ty,
                            &[],
                            source.span,
                            "implicit secondary `super()` delegation",
                        ) else {
                            return;
                        };
                        base
                    }
                    None => hir::BaseInitialization::Root,
                };
                self.set_class_secondary_delegation(
                    constructor,
                    hir::ClassSecondaryDelegation::Terminal {
                        base,
                        common_initialization: Vec::new(),
                    },
                );
            }
        }
    }

    fn lower_class_this_edge(
        &mut self,
        owner: ClassId,
        source: hir::ClassConstructorId,
        arguments: &[ast::CallArgument],
        span: ast::Span,
        context: &str,
    ) -> Option<(
        hir::ClassConstructorApplicationId,
        hir::ConstructorArguments,
    )> {
        let owner_application = self.classes[owner].self_application;
        let type_arguments = self.class_applications[owner_application].arguments.clone();
        let explicit_type_arguments = type_arguments
            .iter()
            .skip(self.enclosing_parameter_count(owner))
            .copied()
            .map(|ty| crate::expr::ResolvedCallTypeArgument::Explicit { ty, span })
            .collect::<Vec<_>>();
        let candidates = self
            .source_class_constructors(owner)
            .map(NominalConstructorSource::Class)
            .collect::<Vec<_>>();
        let name = self.classes[owner].name.clone();
        let resolved = self.with_constructor_source_body(
            source,
            hir::EditingConstructionBody::Delegation(hir::SourceContextConstructor::Class(source)),
            context,
            self.class_constructors[source].safety,
            |this, sink| {
                this.observe_constructor_delegation(
                    hir::SourceContextConstructor::Class(source),
                    span,
                );
                let resolved = this.resolve_nominal_constructor_overload(
                    &name,
                    &candidates,
                    crate::constructor_resolution::NominalConstructorCall {
                        enclosing: None,
                        explicit_type_args: &explicit_type_arguments,
                        expected_type_args: Some(&type_arguments),
                        arguments,
                        span,
                    },
                    sink,
                )?;
                this.observe_completed_delegation(
                    span,
                    this.class_applications[owner_application].canonical_type,
                );
                Some(resolved)
            },
        )?;
        let NominalConstructorSource::Class(target) = resolved.value.source else {
            unreachable!("a `this` edge contains class constructor candidates")
        };
        debug_assert_eq!(resolved.value.type_args, type_arguments);
        let target = self.class_constructor_application(target, owner_application);
        Some((
            target,
            hir::ConstructorArguments {
                locals: resolved.locals,
                statements: resolved.statements,
                args: resolved.value.args,
            },
        ))
    }

    fn set_class_secondary_delegation(
        &mut self,
        constructor: hir::ClassConstructorId,
        delegation: hir::ClassSecondaryDelegation,
    ) {
        let hir::ClassConstructorKind::Secondary {
            delegation: current,
            ..
        } = &mut self.class_constructors[constructor].kind
        else {
            unreachable!("a secondary edge belongs to a secondary constructor")
        };
        *current = delegation;
    }
}
