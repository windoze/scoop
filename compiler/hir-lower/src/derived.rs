//! Compiler-derived value-type members.
//!
//! A declaration in this module is only stable callable identity and a typed
//! conditional signature. Every requested application later receives a
//! complete ordinary HIR body whose nested calls have already been resolved.

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::{CallableCandidate, FnParam, FnSig, Function, FunctionKind, Lowerer, Owner, Type};

pub(crate) enum DerivedEqualityCandidate {
    Nominal {
        overload: CallableCandidate,
        application: hir::DerivedEqualityApplicationId,
    },
    Structural {
        function: hir::FunctionId,
        application: hir::DerivedEqualityApplicationId,
    },
}

impl Lowerer {
    pub(crate) fn declare_derived_equality_methods(&mut self) {
        let structs = self.structs.iter().map(|(id, _)| id).collect::<Vec<_>>();
        for id in structs {
            if matches!(
                self.structs[id].representation,
                hir::StructRepresentation::Intrinsic(_)
            ) {
                continue;
            }
            let owner = Owner::Struct(id);
            if self.has_same_type_equals(owner) {
                continue;
            }
            let function = self.declare_derived_equality_method(owner);
            self.structs[id].derived_equality = Some(function);
        }

        let enums = self.enums.iter().map(|(id, _)| id).collect::<Vec<_>>();
        for id in enums {
            let owner = Owner::Enum(id);
            if self.has_same_type_equals(owner) {
                continue;
            }
            let function = self.declare_derived_equality_method(owner);
            self.enums[id].derived_equality = Some(function);
        }
    }

    fn has_same_type_equals(&mut self, owner: Owner) -> bool {
        let owner_ty = self.owner_ty(owner);
        let methods = match owner {
            Owner::Struct(id) => &self.structs[id].methods,
            Owner::Enum(id) => &self.enums[id].methods,
            Owner::Class(id) => &self.classes[id].methods,
            Owner::Interface(id) => &self.interface_methods[&id],
        };
        methods.iter().copied().any(|function| {
            let signature = &self.signatures[&function];
            signature.operator == Some(hir::OperatorKind::Equals)
                && matches!(signature.params.as_slice(), [parameter] if self.types_equal(parameter.ty, owner_ty))
        })
    }

    fn declare_derived_equality_method(&mut self, owner: Owner) -> hir::FunctionId {
        let owner_ty = self.owner_ty(owner);
        let owner_parameters = self.owner_type_params(owner);
        let span = match owner {
            Owner::Struct(id) => self.structs[id].span,
            Owner::Enum(id) => self.enums[id].span,
            Owner::Class(id) => self.classes[id].span,
            Owner::Interface(id) => self.interfaces[id].span,
        };
        let mut attributes = hir::FunctionAttributes::default();
        if self.requires_unsafe_use(owner_ty) {
            attributes.safety = hir::Safety::Unsafe;
        }
        // Derived declarations do not own a body-local arena. Every requested
        // application supplies one whose first two locals are structurally
        // the receiver and argument, so the declaration can name those slots
        // without consuming unrelated lexical binding identities.
        let this = hir::LocalId::from_raw(0.into());
        let other = hir::LocalId::from_raw(1.into());
        let function = self.functions.alloc(Function {
            name: format!("{}.equals", owner.describe_name(self)),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: vec![
                hir::Param {
                    name: "this".to_string(),
                    ty: owner_ty,
                    local: this,
                },
                hir::Param {
                    name: "other".to_string(),
                    ty: owner_ty,
                    local: other,
                },
            ],
            return_ty: self.boolean,
            attributes,
            kind: FunctionKind::DerivedEquality,
            method: Some(hir::Method {
                owner: owner_ty,
                modifier: hir::MethodModifier::Final,
                dispatch: hir::MethodDispatch::Direct,
                operator: Some(hir::OperatorKind::Equals),
            }),
            span,
        });
        self.register_method_parameters(function, owner_parameters.clone(), Vec::new());
        self.function_owner.insert(function, owner);
        let file = match owner {
            Owner::Struct(id) => self.struct_files[&id],
            Owner::Enum(id) => self.enum_files[&id],
            Owner::Class(id) => self.class_files[&id],
            Owner::Interface(id) => self.interface_files[&id],
        };
        self.function_files.insert(function, file);
        self.signatures.insert(
            function,
            FnSig {
                is_suspend: false,
                operator: Some(hir::OperatorKind::Equals),
                attributes,
                owner_type_param_count: owner_parameters.len(),
                type_params: owner_parameters,
                params: vec![FnParam {
                    name: ast::Ident {
                        text: "other".to_string(),
                        span,
                    },
                    ty: owner_ty,
                }],
                return_ty: self.boolean,
            },
        );
        function
    }

    pub(crate) fn derived_equality_candidate(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> Result<Option<DerivedEqualityCandidate>, String> {
        let nominal = match self.types[ty].clone() {
            Type::Struct(application) => {
                let declaration = &self.structs[self.struct_applications[application].template];
                let Some(function) = declaration.derived_equality else {
                    return Ok(None);
                };
                Some((function, hir::MethodOwnerApplication::Struct(application)))
            }
            Type::Enum(application) => {
                let declaration = &self.enums[self.enum_applications[application].template];
                let Some(function) = declaration.derived_equality else {
                    return Ok(None);
                };
                Some((function, hir::MethodOwnerApplication::Enum(application)))
            }
            Type::Unit | Type::Tuple(_) => None,
            _ => return Ok(None),
        };
        if let Some((function, owner)) = nominal {
            let application = self.ensure_derived_equality_application(
                ty,
                function,
                hir::DerivedEqualityOrigin::Nominal(owner),
                span,
                &mut Vec::new(),
            )?;
            return Ok(Some(DerivedEqualityCandidate::Nominal {
                overload: CallableCandidate::method(function, owner),
                application,
            }));
        }

        let (function, application) =
            self.ensure_structural_derived_equality_application(ty, span, &mut Vec::new())?;
        Ok(Some(DerivedEqualityCandidate::Structural {
            function,
            application,
        }))
    }

    fn ensure_structural_derived_equality_application(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<(hir::FunctionId, hir::DerivedEqualityApplicationId), String> {
        if let Some(&application) = self.derived_equality_application_by_type.get(&ty) {
            return Ok((
                self.derived_equality_applications[application].function,
                application,
            ));
        }
        let function = self.declare_structural_derived_equality_method(ty, span);
        let application = self.ensure_derived_equality_application(
            ty,
            function,
            hir::DerivedEqualityOrigin::Structural(ty),
            span,
            stack,
        )?;
        Ok((function, application))
    }

    fn declare_structural_derived_equality_method(
        &mut self,
        owner_ty: hir::TypeId,
        span: ast::Span,
    ) -> hir::FunctionId {
        let mut attributes = hir::FunctionAttributes::default();
        if self.requires_unsafe_use(owner_ty) {
            attributes.safety = hir::Safety::Unsafe;
        }
        let this = hir::LocalId::from_raw(0.into());
        let other = hir::LocalId::from_raw(1.into());
        let function = self.functions.alloc(Function {
            name: format!("{}.equals", self.type_name(owner_ty)),
            genericity: hir::FunctionGenericity::Plain,
            is_suspend: false,
            params: vec![
                hir::Param {
                    name: "this".to_string(),
                    ty: owner_ty,
                    local: this,
                },
                hir::Param {
                    name: "other".to_string(),
                    ty: owner_ty,
                    local: other,
                },
            ],
            return_ty: self.boolean,
            attributes,
            kind: FunctionKind::DerivedEquality,
            method: Some(hir::Method {
                owner: owner_ty,
                modifier: hir::MethodModifier::Final,
                dispatch: hir::MethodDispatch::Direct,
                operator: Some(hir::OperatorKind::Equals),
            }),
            span,
        });
        self.function_files.insert(function, self.current_file);
        self.signatures.insert(
            function,
            FnSig {
                is_suspend: false,
                operator: Some(hir::OperatorKind::Equals),
                attributes,
                owner_type_param_count: 0,
                type_params: Vec::new(),
                params: vec![FnParam {
                    name: ast::Ident {
                        text: "other".to_string(),
                        span,
                    },
                    ty: owner_ty,
                }],
                return_ty: self.boolean,
            },
        );
        function
    }

    fn ensure_derived_equality_application(
        &mut self,
        ty: hir::TypeId,
        function: hir::FunctionId,
        origin: hir::DerivedEqualityOrigin,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::DerivedEqualityApplicationId, String> {
        if let Some(&application) = self.derived_equality_application_by_type.get(&ty) {
            return Ok(application);
        }
        if stack.contains(&ty) {
            return Err(format!(
                "recursive value layout reaches `{}` without crossing a reference boundary",
                self.type_name(ty)
            ));
        }
        stack.push(ty);
        let body = self.build_derived_equality_body(ty, span, stack);
        let popped = stack.pop();
        debug_assert_eq!(popped, Some(ty));
        let body = body?;
        let application =
            self.derived_equality_applications
                .alloc(hir::DerivedEqualityApplication {
                    function,
                    origin,
                    owner_ty: ty,
                    attributes: self.functions[function].attributes,
                    span,
                    body,
                });
        self.derived_equality_application_by_type
            .insert(ty, application);
        Ok(application)
    }

    fn build_derived_equality_body(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::Body, String> {
        let mut locals = Arena::new();
        let this = self.alloc_derived_local(&mut locals, "this", ty);
        let other = self.alloc_derived_local(&mut locals, "other", ty);
        let this_expr = local_expr(this, ty, span);
        let other_expr = local_expr(other, ty, span);
        let statements = match self.types[ty].clone() {
            Type::Unit => vec![return_statement(bool_expr(true, self.boolean, span), span)],
            Type::Tuple(elements) => {
                let mut comparisons = Vec::with_capacity(elements.len());
                for (index, element) in elements.into_iter().enumerate() {
                    let field = hir::FieldRef::TupleIndex(index as u32);
                    comparisons.push(self.build_derived_field_comparison(
                        element,
                        field_expr(this_expr.clone(), field, element, span),
                        field_expr(other_expr.clone(), field, element, span),
                        &format!("{}._{}", self.type_name(ty), index + 1),
                        span,
                        stack,
                    )?);
                }
                vec![return_statement(
                    fold_conjunction(comparisons, self.boolean, span),
                    span,
                )]
            }
            Type::Struct(application) => {
                let value = self.struct_applications[application].clone();
                let fields = self.structs[value.template].semantic_fields().to_vec();
                let mut comparisons = Vec::with_capacity(fields.len());
                for (index, field) in fields.into_iter().enumerate() {
                    let field_ty = self.instantiate_ty(field.ty, &value.arguments);
                    let field_ref = hir::FieldRef::StructField {
                        application,
                        index: index as u32,
                    };
                    let left = field_expr(this_expr.clone(), field_ref, field_ty, span);
                    let right = field_expr(other_expr.clone(), field_ref, field_ty, span);
                    let path = format!("{}.{}", self.structs[value.template].name, field.name);
                    comparisons.push(self.build_derived_field_comparison(
                        field_ty, left, right, &path, span, stack,
                    )?);
                }
                vec![return_statement(
                    fold_conjunction(comparisons, self.boolean, span),
                    span,
                )]
            }
            Type::Enum(application) => {
                let value = self.enum_applications[application].clone();
                let declaration = self.enums[value.template].clone();
                let mut arms = Vec::with_capacity(declaration.variants.len());
                for (variant_index, variant) in declaration.variants.into_iter().enumerate() {
                    let mut left_fields = Vec::with_capacity(variant.fields.len());
                    let mut right_fields = Vec::with_capacity(variant.fields.len());
                    let mut comparisons = Vec::with_capacity(variant.fields.len());
                    for (field_index, field) in variant.fields.into_iter().enumerate() {
                        let field_ty = self.instantiate_ty(field.ty, &value.arguments);
                        let left = self.alloc_derived_local(
                            &mut locals,
                            &format!("$left.{variant_index}.{field_index}"),
                            field_ty,
                        );
                        let right = self.alloc_derived_local(
                            &mut locals,
                            &format!("$right.{variant_index}.{field_index}"),
                            field_ty,
                        );
                        left_fields
                            .push((field_index as u32, hir::Pattern::Binding { local: left }));
                        right_fields
                            .push((field_index as u32, hir::Pattern::Binding { local: right }));
                        let field_name = if field.name.is_empty() {
                            format!("_{}", field_index + 1)
                        } else {
                            field.name
                        };
                        let path = format!("{}.{}.{}", declaration.name, variant.name, field_name);
                        comparisons.push(self.build_derived_field_comparison(
                            field_ty,
                            local_expr(left, field_ty, span),
                            local_expr(right, field_ty, span),
                            &path,
                            span,
                            stack,
                        )?);
                    }
                    let equal = fold_conjunction(comparisons, self.boolean, span);
                    let inner = hir::Statement {
                        kind: hir::StatementKind::When(hir::When {
                            subject: other_expr.clone(),
                            arms: vec![hir::WhenArm {
                                pattern: hir::Pattern::Variant {
                                    application,
                                    variant: variant_index as u32,
                                    fields: right_fields,
                                },
                                guard: None,
                                body: vec![return_statement(equal, span)],
                                span,
                            }],
                            else_body: Some(vec![return_statement(
                                bool_expr(false, self.boolean, span),
                                span,
                            )]),
                        }),
                        span,
                    };
                    arms.push(hir::WhenArm {
                        pattern: hir::Pattern::Variant {
                            application,
                            variant: variant_index as u32,
                            fields: left_fields,
                        },
                        guard: None,
                        body: vec![inner],
                        span,
                    });
                }
                vec![hir::Statement {
                    kind: hir::StatementKind::When(hir::When {
                        subject: this_expr,
                        arms,
                        else_body: None,
                    }),
                    span,
                }]
            }
            _ => unreachable!("only Unit, tuple, struct and enum have derived equality"),
        };
        Ok(hir::Body { locals, statements })
    }

    fn build_derived_field_comparison(
        &mut self,
        ty: hir::TypeId,
        lhs: hir::Expr,
        rhs: hir::Expr,
        path: &str,
        span: ast::Span,
        stack: &mut Vec<hir::TypeId>,
    ) -> Result<hir::Expr, String> {
        match self.types[ty].clone() {
            Type::Unit | Type::Tuple(_) => {
                let (_, application) =
                    self.ensure_structural_derived_equality_application(ty, span, stack)?;
                Ok(derived_call(lhs, rhs, application, self.boolean, span))
            }
            Type::Struct(application) => {
                let function =
                    self.structs[self.struct_applications[application].template].derived_equality;
                if let Some(function) = function {
                    let owner = hir::MethodOwnerApplication::Struct(application);
                    match self.ensure_derived_equality_application(
                        ty,
                        function,
                        hir::DerivedEqualityOrigin::Nominal(owner),
                        span,
                        stack,
                    ) {
                        Ok(application) => {
                            return Ok(derived_call(lhs, rhs, application, self.boolean, span));
                        }
                        Err(reason) => {
                            return self
                                .resolve_explicit_field_equality(ty, lhs, rhs, path, span)
                                .map_err(|_| reason);
                        }
                    }
                }
                self.resolve_explicit_field_equality(ty, lhs, rhs, path, span)
            }
            Type::Enum(application) => {
                let function =
                    self.enums[self.enum_applications[application].template].derived_equality;
                if let Some(function) = function {
                    let owner = hir::MethodOwnerApplication::Enum(application);
                    match self.ensure_derived_equality_application(
                        ty,
                        function,
                        hir::DerivedEqualityOrigin::Nominal(owner),
                        span,
                        stack,
                    ) {
                        Ok(application) => {
                            return Ok(derived_call(lhs, rhs, application, self.boolean, span));
                        }
                        Err(reason) => {
                            return self
                                .resolve_explicit_field_equality(ty, lhs, rhs, path, span)
                                .map_err(|_| reason);
                        }
                    }
                }
                self.resolve_explicit_field_equality(ty, lhs, rhs, path, span)
            }
            _ => self.resolve_explicit_field_equality(ty, lhs, rhs, path, span),
        }
    }

    fn resolve_explicit_field_equality(
        &mut self,
        ty: hir::TypeId,
        lhs: hir::Expr,
        rhs: hir::Expr,
        path: &str,
        span: ast::Span,
    ) -> Result<hir::Expr, String> {
        let candidates = self
            .methods_by_name(ty, "equals")
            .into_iter()
            .filter(|candidate| {
                self.signatures[&candidate.function].operator == Some(hir::OperatorKind::Equals)
            })
            .collect::<Vec<_>>();
        let mut applicable = Vec::new();
        for candidate in candidates {
            let arguments = self.callable_candidate_owner_arguments(&candidate);
            let signature = self.signatures[&candidate.function].clone();
            debug_assert_eq!(
                signature.type_params.len(),
                signature.owner_type_param_count
            );
            let parameter = self.instantiate_ty(signature.params[0].ty, &arguments);
            if self.is_subtype(rhs.ty, parameter) {
                applicable.push((candidate, arguments, parameter));
            }
        }
        let mut winners = Vec::new();
        for index in 0..applicable.len() {
            let parameter = applicable[index].2;
            let dominates_all = (0..applicable.len())
                .all(|other| index == other || self.is_subtype(parameter, applicable[other].2));
            if dominates_all {
                winners.push(index);
            }
        }
        let [winner] = winners.as_slice() else {
            let reason = if applicable.is_empty() {
                format!(
                    "field `{path}` of type `{}` has no applicable member operator `equals`",
                    self.type_name(ty)
                )
            } else {
                format!(
                    "field `{path}` has an ambiguous member operator `equals` for type `{}`",
                    self.type_name(ty)
                )
            };
            return Err(reason);
        };
        let (candidate, arguments, parameter) = applicable.swap_remove(*winner);
        let callable = self.materialize_candidate_callable(&candidate, &arguments);
        let callee = self.materialize_method_callee(candidate.source, callable, &arguments);
        Ok(hir::Expr {
            kind: hir::ExprKind::MethodCall {
                receiver: Box::new(lhs),
                callee,
                args: vec![self.adapt_to(rhs, parameter)],
            },
            ty: self.boolean,
            span,
        })
    }

    fn alloc_derived_local(
        &mut self,
        locals: &mut Arena<hir::Local>,
        name: &str,
        ty: hir::TypeId,
    ) -> hir::LocalId {
        locals.alloc(hir::Local {
            binding: self.fresh_binding(),
            name: name.to_string(),
            ty,
            mutable: false,
        })
    }
}

fn derived_call(
    receiver: hir::Expr,
    other: hir::Expr,
    application: hir::DerivedEqualityApplicationId,
    boolean: hir::TypeId,
    span: ast::Span,
) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::MethodCall {
            receiver: Box::new(receiver),
            callee: hir::MethodCallee::DerivedEquality(application),
            args: vec![other],
        },
        ty: boolean,
        span,
    }
}

fn local_expr(local: hir::LocalId, ty: hir::TypeId, span: ast::Span) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::Local(local),
        ty,
        span,
    }
}

fn field_expr(
    receiver: hir::Expr,
    field: hir::FieldRef,
    ty: hir::TypeId,
    span: ast::Span,
) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::FieldAccess {
            receiver: Box::new(receiver),
            field,
        },
        ty,
        span,
    }
}

fn bool_expr(value: bool, boolean: hir::TypeId, span: ast::Span) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::BoolLiteral(value),
        ty: boolean,
        span,
    }
}

fn fold_conjunction(
    comparisons: Vec<hir::Expr>,
    boolean: hir::TypeId,
    span: ast::Span,
) -> hir::Expr {
    comparisons
        .into_iter()
        .reduce(|lhs, rhs| hir::Expr {
            kind: hir::ExprKind::Binary {
                op: hir::BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: boolean,
            span,
        })
        .unwrap_or_else(|| bool_expr(true, boolean, span))
}

fn return_statement(value: hir::Expr, span: ast::Span) -> hir::Statement {
    hir::Statement {
        kind: hir::StatementKind::Return { value: Some(value) },
        span,
    }
}
