use super::*;

use crate::call_resolution::candidates::NominalConstructorSource;

mod context;
mod imported;
mod validation;

pub(crate) use context::ConstructorSource;

impl Lowerer {
    pub(crate) fn struct_primary_constructor(
        &self,
        structure: hir::StructId,
    ) -> Option<hir::StructConstructorId> {
        self.structs[structure]
            .constructors
            .iter()
            .copied()
            .find(|constructor| {
                matches!(
                    self.struct_constructors[*constructor].kind,
                    hir::StructConstructorKind::Primary
                )
            })
    }

    pub(crate) fn resolve_constructor_graphs(
        &mut self,
        classes: &[(ClassId, &ast::ClassDecl, usize)],
        structs: &[(hir::StructId, &ast::StructDecl, usize)],
        objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        self.check_duplicate_constructor_signatures(classes, structs);
        let outer_owner = self.current_owner;
        for &(class, declaration, file) in classes {
            self.current_owner = Some(Owner::Class(class));
            self.current_file = file;
            let diagnostics_before_edges = self.diagnostics.len();
            self.resolve_class_constructor_edges(class, declaration);
            if self.diagnostics.len() == diagnostics_before_edges {
                self.check_class_constructor_cycles(class);
            }
        }
        for &(structure, declaration, file) in structs {
            self.current_owner = Some(Owner::Struct(structure));
            self.current_file = file;
            let diagnostics_before_edges = self.diagnostics.len();
            self.resolve_struct_constructor_edges(structure, declaration);
            if self.diagnostics.len() == diagnostics_before_edges {
                self.check_struct_constructor_cycles(structure);
            }
        }
        for &(object, declaration, file) in objects {
            self.current_owner = Some(Owner::Object(object));
            self.current_file = file;
            self.resolve_object_base(object, declaration);
        }
        self.current_owner = outer_owner;
    }

    fn resolve_object_base(
        &mut self,
        object: hir::ObjectId,
        source: crate::declarations::ObjectSource<'_>,
    ) {
        let backing = self.objects[object].backing_class;
        let constructor = self.classes[backing]
            .constructors
            .first()
            .copied()
            .expect("every object has one hidden primary constructor");
        let Some(base_ty) = self.classes[backing].base_class else {
            self.set_primary_base(constructor, hir::BaseInitialization::Root);
            return;
        };
        let specification = source.supertypes().iter().find(|specification| {
            self.resolve_type_ref(&specification.ty)
                .is_some_and(|ty| self.types_equal(ty, base_ty))
        });
        let Some(specification) = specification.cloned() else {
            return;
        };
        let arguments = specification
            .constructor_arguments
            .as_deref()
            .unwrap_or(&[]);
        let unit = self.singleton_values[self.objects[object].singleton_value].initialization;
        let previous_unit = self.current_initialization_unit.replace(unit);
        let Some(base) = self.lower_base_initialization(
            constructor,
            base_ty,
            arguments,
            specification.span,
            "object base constructor delegation",
        ) else {
            self.current_initialization_unit = previous_unit;
            return;
        };
        self.current_initialization_unit = previous_unit;
        self.set_primary_base(constructor, base);
    }

    fn resolve_class_constructor_edges(&mut self, id: ClassId, decl: &ast::ClassDecl) {
        if !self.classes[id].is_declared() {
            return;
        }
        let constructors = self.classes[id].constructors.clone();
        let primary = constructors.iter().copied().find(|constructor| {
            matches!(
                self.class_constructors[*constructor].kind,
                hir::ClassConstructorKind::Primary { .. }
            )
        });
        if let Some(primary) = primary {
            self.resolve_primary_base(id, primary, decl);
        } else if let Some(spec) = self.source_base_spec(id, decl)
            && spec.constructor_arguments.is_some()
        {
            self.error(
                spec.span,
                "a class without a primary constructor cannot pass base arguments in its header"
                    .into(),
            );
        }

        let secondary_ids = constructors
            .into_iter()
            .filter(|constructor| {
                matches!(
                    self.class_constructors[*constructor].kind,
                    hir::ClassConstructorKind::Secondary { .. }
                )
            })
            .collect::<Vec<_>>();
        for (constructor, source) in secondary_ids.into_iter().zip(decl.secondary_constructors()) {
            self.resolve_class_secondary_edge(id, primary, constructor, source);
        }
    }

    fn resolve_primary_base(
        &mut self,
        owner: ClassId,
        primary: hir::ClassConstructorId,
        decl: &ast::ClassDecl,
    ) {
        let Some(base_ty) = self.classes[owner].base_class else {
            self.set_primary_base(primary, hir::BaseInitialization::Root);
            return;
        };
        let Some(spec) = self.source_base_spec(owner, decl).cloned() else {
            return;
        };
        let arguments = spec.constructor_arguments.as_deref().unwrap_or(&[]);
        let Some(base) = self.lower_base_initialization(
            primary,
            base_ty,
            arguments,
            spec.span,
            "primary constructor delegation",
        ) else {
            return;
        };
        self.set_primary_base(primary, base);
    }

    pub(crate) fn set_primary_base(
        &mut self,
        constructor: hir::ClassConstructorId,
        base: hir::BaseInitialization,
    ) {
        let hir::ClassConstructorKind::Primary { base: current, .. } =
            &mut self.class_constructors[constructor].kind
        else {
            unreachable!("a primary base belongs to a primary constructor")
        };
        *current = base;
    }

    fn resolve_class_secondary_edge(
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
            .copied()
            .map(|ty| crate::expr::ResolvedCallTypeArgument::Explicit { ty, span })
            .collect::<Vec<_>>();
        let candidates = self.classes[owner]
            .constructors
            .iter()
            .copied()
            .map(NominalConstructorSource::Class)
            .collect::<Vec<_>>();
        let name = self.classes[owner].name.clone();
        let resolved = self.with_constructor_expression_context(
            source,
            context,
            self.class_constructors[source].safety,
            |this, sink| {
                this.resolve_nominal_constructor_overload(
                    &name,
                    &candidates,
                    crate::constructor_resolution::NominalConstructorCall {
                        explicit_type_args: &explicit_type_arguments,
                        expected_type_args: None,
                        arguments,
                        span,
                    },
                    sink,
                )
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

    pub(crate) fn lower_base_initialization(
        &mut self,
        source: hir::ClassConstructorId,
        base_ty: TypeId,
        arguments: &[ast::CallArgument],
        span: ast::Span,
        context: &str,
    ) -> Option<hir::BaseInitialization> {
        if matches!(self.types[base_ty], Type::ImportedClass(_)) {
            return self
                .lower_imported_base_initialization(source, base_ty, arguments, span, context);
        }
        let Type::Class(base_application) = self.types[base_ty] else {
            unreachable!("a direct base type is a class application")
        };
        let base = self.class_applications[base_application].clone();
        let candidates = self.classes[base.template]
            .constructors
            .iter()
            .copied()
            .map(NominalConstructorSource::Class)
            .collect::<Vec<_>>();
        let name = self.classes[base.template].name.clone();
        let type_arguments = base.arguments;
        let explicit_type_arguments = type_arguments
            .iter()
            .copied()
            .map(|ty| crate::expr::ResolvedCallTypeArgument::Explicit { ty, span })
            .collect::<Vec<_>>();
        let resolved = self.with_constructor_expression_context(
            source,
            context,
            self.class_constructors[source].safety,
            |this, sink| {
                this.resolve_nominal_constructor_overload(
                    &name,
                    &candidates,
                    crate::constructor_resolution::NominalConstructorCall {
                        explicit_type_args: &explicit_type_arguments,
                        expected_type_args: None,
                        arguments,
                        span,
                    },
                    sink,
                )
            },
        )?;
        let NominalConstructorSource::Class(target) = resolved.value.source else {
            unreachable!("a `super` edge contains class constructor candidates")
        };
        debug_assert_eq!(resolved.value.type_args, type_arguments);
        let target = self.class_constructor_application(target, base_application);
        Some(hir::BaseInitialization::Super {
            target: hir::BaseInitializerTarget::Local(target),
            arguments: hir::ConstructorArguments {
                locals: resolved.locals,
                statements: resolved.statements,
                args: resolved.value.args,
            },
        })
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

    fn resolve_struct_constructor_edges(&mut self, id: hir::StructId, decl: &ast::StructDecl) {
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
            let Some(resolved) = self.with_constructor_expression_context(
                ConstructorSource::Struct(constructor),
                "struct secondary `this` delegation",
                self.struct_constructors[constructor].safety,
                |this, sink| {
                    this.resolve_nominal_constructor_overload(
                        &name,
                        &candidates,
                        crate::constructor_resolution::NominalConstructorCall {
                            explicit_type_args: &explicit_type_arguments,
                            expected_type_args: None,
                            arguments: arguments.as_slice(),
                            span: *span,
                        },
                        sink,
                    )
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

    fn source_base_spec<'a>(
        &mut self,
        owner: ClassId,
        declaration: &'a ast::ClassDecl,
    ) -> Option<&'a ast::SupertypeSpec> {
        let base = self.classes[owner].base_class?;
        let outer = std::mem::replace(
            &mut self.type_params_in_scope,
            self.classes[owner].type_params.clone(),
        );
        let result = declaration.supertypes.iter().find(|spec| {
            self.resolve_type_ref(&spec.ty)
                .is_some_and(|ty| self.types_equal(ty, base))
        });
        self.type_params_in_scope = outer;
        result
    }
}
