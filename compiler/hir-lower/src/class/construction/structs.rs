use super::*;

impl Lowerer {
    pub(super) fn resolve_struct_constructor_edges(
        &mut self,
        id: hir::StructId,
        decl: ast::StructSource<'_>,
    ) {
        if !matches!(
            self.structs[id].representation,
            hir::StructRepresentation::Declared(_)
        ) {
            return;
        }
        let secondary_ids = self.structs[id]
            .constructors
            .iter()
            .copied()
            .filter(|constructor| {
                matches!(
                    self.struct_constructors[*constructor].kind,
                    hir::StructConstructorKind::Secondary { .. }
                )
            })
            .collect::<Vec<_>>();
        for (constructor, source) in secondary_ids.into_iter().zip(decl.secondary_constructors()) {
            let Some(ast::ConstructorDelegation::This {
                arguments, span, ..
            }) = source.delegation.as_ref()
            else {
                let message = if matches!(
                    source.delegation,
                    Some(ast::ConstructorDelegation::Super { .. })
                ) {
                    "a struct secondary constructor cannot use `super(...)`; it must delegate with `this(...)`"
                } else {
                    "a struct secondary constructor must declare `this(...)` delegation"
                };
                self.error(source.span, message.into());
                continue;
            };
            let owner_application = self.structs[id].self_application;
            let type_arguments = self.struct_applications[owner_application]
                .arguments
                .clone();
            let explicit_type_arguments = type_arguments
                .iter()
                .copied()
                .map(|ty| crate::expr::ResolvedCallTypeArgument::Explicit { ty, span: *span })
                .collect::<Vec<_>>();
            let candidates = self.structs[id]
                .constructors
                .iter()
                .copied()
                .map(NominalConstructorSource::Struct)
                .collect::<Vec<_>>();
            let name = self.structs[id].name.clone();
            let Some(resolved) = self.with_constructor_source_body(
                ConstructorSource::Struct(constructor),
                hir::EditingConstructionBody::Delegation(hir::SourceContextConstructor::Struct(
                    constructor,
                )),
                "struct secondary `this` delegation",
                self.struct_constructors[constructor].safety,
                |this, sink| {
                    this.observe_constructor_delegation(
                        hir::SourceContextConstructor::Struct(constructor),
                        *span,
                    );
                    let resolved = this.resolve_nominal_constructor_overload(
                        &name,
                        &candidates,
                        crate::constructor_resolution::NominalConstructorCall {
                            enclosing: None,
                            explicit_type_args: &explicit_type_arguments,
                            expected_type_args: None,
                            arguments: arguments.as_slice(),
                            span: *span,
                        },
                        sink,
                    )?;
                    this.observe_completed_delegation(
                        *span,
                        this.struct_applications[owner_application].canonical_type,
                    );
                    Some(resolved)
                },
            ) else {
                continue;
            };
            let NominalConstructorSource::Struct(target) = resolved.value.source else {
                unreachable!("a struct `this` edge contains struct candidates")
            };
            debug_assert_eq!(resolved.value.type_args, type_arguments);
            let target = self.struct_constructor_application(target, owner_application);
            let hir::StructConstructorKind::Secondary { delegation, .. } =
                &mut self.struct_constructors[constructor].kind
            else {
                unreachable!("the source constructor is secondary")
            };
            *delegation = hir::StructConstructorDelegation {
                target,
                arguments: hir::ConstructorArguments {
                    locals: resolved.locals,
                    statements: resolved.statements,
                    args: resolved.value.args,
                },
            };
        }
    }
}
