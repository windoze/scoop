use super::*;

use std::collections::{HashMap, HashSet};

use crate::call_resolution::candidates::NominalConstructorSource;

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
    ) {
        self.check_duplicate_constructor_signatures(classes, structs);
        for &(class, declaration, file) in classes {
            self.current_file = file;
            let diagnostics_before_edges = self.diagnostics.len();
            self.resolve_class_constructor_edges(class, declaration);
            if self.diagnostics.len() == diagnostics_before_edges {
                self.check_class_constructor_cycles(class);
            }
        }
        for &(structure, declaration, file) in structs {
            self.current_file = file;
            let diagnostics_before_edges = self.diagnostics.len();
            self.resolve_struct_constructor_edges(structure, declaration);
            if self.diagnostics.len() == diagnostics_before_edges {
                self.check_struct_constructor_cycles(structure);
            }
        }
    }

    fn check_duplicate_constructor_signatures(
        &mut self,
        classes: &[(ClassId, &ast::ClassDecl, usize)],
        structs: &[(hir::StructId, &ast::StructDecl, usize)],
    ) {
        for &(class, _, file) in classes {
            self.current_file = file;
            let constructors = self.classes[class].constructors.clone();
            for (index, &constructor) in constructors.iter().enumerate() {
                for &previous in &constructors[..index] {
                    let left = self.class_constructors[constructor].parameters.clone();
                    let right = self.class_constructors[previous].parameters.clone();
                    if self.same_constructor_parameter_types(&left, &right) {
                        let signature = self.class_constructor_signature(constructor);
                        self.error(
                            self.class_constructors[constructor].span,
                            format!(
                                "duplicate constructor signature `{signature}` in class `{}`",
                                self.classes[class].name
                            ),
                        );
                    }
                }
            }
        }
        for &(structure, _, file) in structs {
            self.current_file = file;
            let constructors = self.structs[structure].constructors.clone();
            for (index, &constructor) in constructors.iter().enumerate() {
                for &previous in &constructors[..index] {
                    let left = self.struct_constructors[constructor].parameters.clone();
                    let right = self.struct_constructors[previous].parameters.clone();
                    if self.same_constructor_parameter_types(&left, &right) {
                        let signature = self.struct_constructor_signature(constructor);
                        self.error(
                            self.struct_constructors[constructor].span,
                            format!(
                                "duplicate constructor signature `{signature}` in struct `{}`",
                                self.structs[structure].name
                            ),
                        );
                    }
                }
            }
        }
    }

    fn same_constructor_parameter_types(
        &mut self,
        left: &[hir::ConstructorParameter],
        right: &[hir::ConstructorParameter],
    ) -> bool {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| self.types_equal(left.ty, right.ty))
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

    fn set_primary_base(
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
        let resolved =
            self.with_constructor_expression_context(source, context, |this, sink| {
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
            })?;
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

    fn lower_base_initialization(
        &mut self,
        source: hir::ClassConstructorId,
        base_ty: TypeId,
        arguments: &[ast::CallArgument],
        span: ast::Span,
        context: &str,
    ) -> Option<hir::BaseInitialization> {
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
        let resolved =
            self.with_constructor_expression_context(source, context, |this, sink| {
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
            })?;
        let NominalConstructorSource::Class(target) = resolved.value.source else {
            unreachable!("a `super` edge contains class constructor candidates")
        };
        debug_assert_eq!(resolved.value.type_args, type_arguments);
        let target = self.class_constructor_application(target, base_application);
        Some(hir::BaseInitialization::Super {
            target,
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

    pub(crate) fn with_constructor_expression_context<T>(
        &mut self,
        source: impl Into<ConstructorSource>,
        context: &str,
        lower: impl FnOnce(&mut Self, &mut Vec<hir::Statement>) -> Option<T>,
    ) -> Option<LoweredConstructorExpression<T>> {
        let source = source.into();
        let (parameters, type_parameters, owner, owner_name) = match source {
            ConstructorSource::Class(constructor) => {
                let declaration = &self.class_constructors[constructor];
                let owner = declaration.owner;
                (
                    declaration.parameters.clone(),
                    self.classes[owner].type_params.clone(),
                    Owner::Class(owner),
                    self.classes[owner].name.clone(),
                )
            }
            ConstructorSource::Struct(constructor) => {
                let declaration = &self.struct_constructors[constructor];
                let owner = declaration.owner;
                (
                    declaration.parameters.clone(),
                    self.structs[owner].type_params.clone(),
                    Owner::Struct(owner),
                    self.structs[owner].name.clone(),
                )
            }
        };
        let outer_type_parameters =
            std::mem::replace(&mut self.type_params_in_scope, type_parameters);
        let outer_constructor_parameters = std::mem::replace(
            &mut self.constructor_params_in_scope,
            parameters
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.clone(),
                        (
                            parameter.id,
                            parameter.ty,
                            self.constructor_parameter_bindings[&parameter.id],
                        ),
                    )
                })
                .collect(),
        );
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_return_ty = std::mem::replace(&mut self.current_return_ty, self.unit);
        let outer_fn_name = std::mem::replace(
            &mut self.current_fn_name,
            format!("<init {owner_name}: {context}>"),
        );
        let outer_owner = self.current_owner.replace(owner);
        let outer_this = self.current_this.take();
        let outer_source_context = self.current_source_context;
        self.set_source_context(self.current_fn_name.clone());
        self.push_scope();
        self.push_suspension_context(SuspensionContext::Forbidden(
            if self.initialization_context.is_some() {
                ForbiddenSuspendContext::ConstructorInitialization
            } else {
                ForbiddenSuspendContext::ConstructorDelegation
            },
        ));

        let mut statements = Vec::new();
        let value = lower(self, &mut statements);
        let locals = std::mem::take(&mut self.locals);

        self.pop_suspension_context();
        self.pop_scope();
        self.current_source_context = outer_source_context;
        self.current_this = outer_this;
        self.current_owner = outer_owner;
        self.current_fn_name = outer_fn_name;
        self.current_return_ty = outer_return_ty;
        self.locals = outer_locals;
        self.constructor_params_in_scope = outer_constructor_parameters;
        self.type_params_in_scope = outer_type_parameters;

        value.map(|value| LoweredConstructorExpression {
            locals,
            statements,
            value,
        })
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

    fn check_class_constructor_cycles(&mut self, owner: ClassId) {
        let mut edges = HashMap::new();
        for &constructor in &self.classes[owner].constructors {
            if let hir::ClassConstructorKind::Secondary {
                delegation: hir::ClassSecondaryDelegation::This { target, .. },
                ..
            } = self.class_constructors[constructor].kind
            {
                edges.insert(
                    constructor,
                    self.class_constructor_applications[target].constructor,
                );
            }
        }
        self.report_class_cycles(&edges);
    }

    fn report_class_cycles(
        &mut self,
        edges: &HashMap<hir::ClassConstructorId, hir::ClassConstructorId>,
    ) {
        let mut reported = HashSet::new();
        let mut starts = edges.keys().copied().collect::<Vec<_>>();
        starts.sort_by_key(|constructor| constructor.into_raw().into_u32());
        for start in starts {
            let mut positions = HashMap::new();
            let mut path = Vec::new();
            let mut current = start;
            while let Some(&next) = edges.get(&current) {
                if let Some(&position) = positions.get(&current) {
                    let cycle = &path[position..];
                    if cycle
                        .iter()
                        .all(|constructor| !reported.contains(constructor))
                    {
                        reported.extend(cycle.iter().copied());
                        let mut signatures = cycle
                            .iter()
                            .map(|constructor| self.class_constructor_signature(*constructor))
                            .collect::<Vec<_>>();
                        signatures.push(signatures[0].clone());
                        self.error(
                            self.class_constructors[current].span,
                            format!("constructor delegation cycle: {}", signatures.join(" -> ")),
                        );
                    }
                    break;
                }
                positions.insert(current, path.len());
                path.push(current);
                current = next;
            }
        }
    }

    fn check_struct_constructor_cycles(&mut self, owner: hir::StructId) {
        let mut edges = HashMap::new();
        for &constructor in &self.structs[owner].constructors {
            if let hir::StructConstructorKind::Secondary { ref delegation, .. } =
                self.struct_constructors[constructor].kind
            {
                edges.insert(
                    constructor,
                    self.struct_constructor_applications[delegation.target].constructor,
                );
            }
        }
        let mut reported = HashSet::new();
        let mut starts = edges.keys().copied().collect::<Vec<_>>();
        starts.sort_by_key(|constructor| constructor.into_raw().into_u32());
        for start in starts {
            let mut positions = HashMap::new();
            let mut path = Vec::new();
            let mut current = start;
            while let Some(&next) = edges.get(&current) {
                if let Some(&position) = positions.get(&current) {
                    let cycle = &path[position..];
                    if cycle
                        .iter()
                        .all(|constructor| !reported.contains(constructor))
                    {
                        reported.extend(cycle.iter().copied());
                        let mut signatures = cycle
                            .iter()
                            .map(|constructor| self.struct_constructor_signature(*constructor))
                            .collect::<Vec<_>>();
                        signatures.push(signatures[0].clone());
                        self.error(
                            self.struct_constructors[current].span,
                            format!("constructor delegation cycle: {}", signatures.join(" -> ")),
                        );
                    }
                    break;
                }
                positions.insert(current, path.len());
                path.push(current);
                current = next;
            }
        }
    }

    fn class_constructor_signature(&mut self, constructor: hir::ClassConstructorId) -> String {
        let declaration = self.class_constructors[constructor].clone();
        let name = self.classes[declaration.owner].name.clone();
        let parameters = declaration
            .parameters
            .iter()
            .map(|parameter| self.type_name(parameter.ty))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}({parameters})")
    }

    fn struct_constructor_signature(&mut self, constructor: hir::StructConstructorId) -> String {
        let declaration = self.struct_constructors[constructor].clone();
        let name = self.structs[declaration.owner].name.clone();
        let parameters = declaration
            .parameters
            .iter()
            .map(|parameter| self.type_name(parameter.ty))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}({parameters})")
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ConstructorSource {
    Class(hir::ClassConstructorId),
    Struct(hir::StructConstructorId),
}

impl From<hir::ClassConstructorId> for ConstructorSource {
    fn from(value: hir::ClassConstructorId) -> Self {
        Self::Class(value)
    }
}

impl From<hir::StructConstructorId> for ConstructorSource {
    fn from(value: hir::StructConstructorId) -> Self {
        Self::Struct(value)
    }
}

pub(crate) struct LoweredConstructorExpression<T> {
    pub(crate) locals: la_arena::Arena<hir::Local>,
    pub(crate) statements: Vec<hir::Statement>,
    pub(crate) value: T,
}
