//! FnLowering expression lowering: literals, unary/binary, type tests, casts, member access.

#![allow(dead_code)]

use super::*;

impl<'a> FnLowering<'a> {
    pub(in crate::mir::lower) fn lower_unresolved_ident(
        &mut self,
        span: Span,
        ty: TypeId,
        name: &str,
    ) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        self.assign(
            span,
            tmp,
            Rvalue::UnresolvedName {
                name: name.to_string(),
            },
        );
        tmp
    }

    /// 生成一个 `Unit` 值，并返回其 local。
    pub(in crate::mir::lower) fn emit_unit(&mut self, span: Span) -> LocalId {
        let tmp = self.push_temp_local(span, self.builtins.unit);
        self.assign(span, tmp, Rvalue::Use(Operand::Const(ConstValue::Unit)));
        tmp
    }

    pub(in crate::mir::lower) fn lower_tuple_lit_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        elements: &[hir::Expr],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let mut lowered = Vec::with_capacity(elements.len());
        let mut field_tys = Vec::with_capacity(elements.len());
        for element in elements {
            let local = self.lower_expr_to_local(element);
            if self.current_is_terminated() {
                return result;
            }
            field_tys.push((None, self.body.locals[local.as_u32() as usize].ty));
            lowered.push(Operand::Local(local));
        }
        self.assign(
            span,
            result,
            Rvalue::MakeTuple {
                elements: lowered,
                transport: self.aggregate_transport(ty, AggregateTransportKind::Tuple, field_tys),
            },
        );
        result
    }

    pub(in crate::mir::lower) fn lower_struct_lit_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        fields: &[hir::StructLitField],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let mut lowered = Vec::with_capacity(fields.len());
        let mut field_tys = Vec::with_capacity(fields.len());
        for field in fields {
            let local = self.lower_expr_to_local(&field.value);
            if self.current_is_terminated() {
                return result;
            }
            field_tys.push((
                Some(field.name.clone()),
                self.body.locals[local.as_u32() as usize].ty,
            ));
            lowered.push(crate::mir::StructLitField {
                span: field.value.span,
                name: field.name.clone(),
                value: Operand::Local(local),
            });
        }
        self.assign(
            span,
            result,
            Rvalue::StructLit {
                fields: lowered,
                transport: self.aggregate_transport(ty, AggregateTransportKind::Struct, field_tys),
            },
        );
        result
    }

    pub(in crate::mir::lower) fn lower_unary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        op: ast::UnaryOp,
        operand: &hir::Expr,
    ) -> LocalId {
        if matches!(op, ast::UnaryOp::Neg)
            && matches!(operand.kind, hir::ExprKind::Literal(hir::LiteralKind::Int))
        {
            let result = self.push_temp_local(span, ty);
            self.assign(span, result, Rvalue::Use(Operand::Const(ConstValue::Int)));
            return result;
        }
        if let Some(result) = self.try_lower_scalar_unary_method_expr(span, ty, op, operand) {
            return result;
        }
        let result = self.push_temp_local(span, ty);
        self.assign(span, result, Rvalue::Todo("missing expr"));
        result
    }

    pub(in crate::mir::lower) fn lower_binary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> LocalId {
        let result_ty = self.binary_result_ty(ty, op);
        match op {
            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                self.lower_short_circuit_binary_expr(span, result_ty, lhs, op, rhs)
            }
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                if let Some(result) =
                    self.try_lower_compare_to_binary_expr(span, result_ty, lhs, op, rhs)
                {
                    return result;
                }
                if let Some(result) =
                    self.try_lower_scalar_binary_method_expr(span, result_ty, lhs, op, rhs)
                {
                    return result;
                }

                let result = self.push_temp_local(span, result_ty);
                self.assign(span, result, Rvalue::Todo("missing expr"));
                result
            }
            ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                if let Some(result) =
                    self.try_lower_string_equality_binary_expr(span, result_ty, lhs, op, rhs)
                {
                    return result;
                }
                if let Some(result) =
                    self.try_lower_scalar_binary_method_expr(span, result_ty, lhs, op, rhs)
                {
                    return result;
                }

                let result = self.push_temp_local(span, result_ty);
                self.assign(span, result, Rvalue::Todo("missing expr"));
                result
            }
            _ => {
                if let Some(result) =
                    self.try_lower_scalar_binary_method_expr(span, result_ty, lhs, op, rhs)
                {
                    return result;
                }

                let result = self.push_temp_local(span, result_ty);
                self.assign(span, result, Rvalue::Todo("missing expr"));
                result
            }
        }
    }

    pub(in crate::mir::lower) fn binary_result_ty(
        &self,
        fallback_ty: TypeId,
        op: ast::BinaryOp,
    ) -> TypeId {
        match op {
            ast::BinaryOp::Lt
            | ast::BinaryOp::Le
            | ast::BinaryOp::Gt
            | ast::BinaryOp::Ge
            | ast::BinaryOp::Eq
            | ast::BinaryOp::Ne
            | ast::BinaryOp::LogAnd
            | ast::BinaryOp::LogOr => self.builtins.bool_,
            _ => fallback_ty,
        }
    }

    pub(in crate::mir::lower) fn runtime_type_test_metadata(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
    ) -> RuntimeTypeTestMetadata {
        RuntimeTypeTestMetadata {
            source_ty,
            target_ty,
            descriptor: self.runtime_type_descriptor_key(target_ty),
            static_fold: self.runtime_type_static_fold(source_ty, target_ty),
            parameterized: self.runtime_type_parameterized_match(target_ty),
        }
    }

    pub(in crate::mir::lower) fn runtime_cast_metadata(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
        result_ty: TypeId,
        op: ast::CastOp,
    ) -> RuntimeCastMetadata {
        let test = self.runtime_type_test_metadata(source_ty, target_ty);
        let (failure, result) = match op {
            ast::CastOp::As => (
                RuntimeCastFailure::Raise {
                    effect_ty: find_raise_runtime_error_effect(self.types),
                    error_fqn: "scoop.core.RuntimeError.ClassCastFailed".to_string(),
                },
                RuntimeCastResult::Target { ty: target_ty },
            ),
            ast::CastOp::AsQ => (
                RuntimeCastFailure::ReturnNone,
                RuntimeCastResult::Option {
                    option_ty: result_ty,
                    some_ty: target_ty,
                },
            ),
        };

        RuntimeCastMetadata {
            test,
            failure,
            result,
        }
    }

    pub(in crate::mir::lower) fn runtime_pattern_type_test_metadata(
        &self,
        subject_ty: TypeId,
        target_ty: TypeId,
    ) -> RuntimePatternTypeTestMetadata {
        let descriptor = self.runtime_type_descriptor_key(target_ty);
        let parameterized = self.runtime_type_parameterized_match(target_ty);
        let match_kind = self.runtime_pattern_match_kind(&descriptor, &parameterized);
        RuntimePatternTypeTestMetadata {
            subject_ty,
            target_ty,
            descriptor,
            match_kind,
            static_fold: self.runtime_type_static_fold(subject_ty, target_ty),
            parameterized,
        }
    }

    pub(in crate::mir::lower) fn runtime_type_descriptor_key(
        &self,
        ty: TypeId,
    ) -> RuntimeTypeDescriptorKey {
        let kind = match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Any) => RuntimeTypeDescriptorKind::Any,
            TypeKind::Ref(RefTypeKind::String) => RuntimeTypeDescriptorKind::String,
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                RuntimeTypeDescriptorKind::Nominal {
                    fqn: nominal.fqn.clone(),
                    kind: self.facts.nominal_kind(&nominal.fqn),
                }
            }
            TypeKind::Ref(RefTypeKind::Function(_)) => RuntimeTypeDescriptorKind::Function,
            TypeKind::Ref(RefTypeKind::Union(_)) => RuntimeTypeDescriptorKind::Union,
            TypeKind::Value(ValueTypeKind::Option(_)) => RuntimeTypeDescriptorKind::Option,
            TypeKind::Value(ValueTypeKind::Tuple(_)) => RuntimeTypeDescriptorKind::Tuple,
            TypeKind::Value(_) => RuntimeTypeDescriptorKind::Value,
            TypeKind::Param(_) => RuntimeTypeDescriptorKind::TypeParam,
            TypeKind::StarProjection(_) => RuntimeTypeDescriptorKind::StarProjection,
        };

        RuntimeTypeDescriptorKey { ty, kind }
    }

    pub(in crate::mir::lower) fn runtime_type_parameterized_match(
        &self,
        ty: TypeId,
    ) -> RuntimeTypeParameterizedMatch {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if nominal.args.is_empty() && nominal.eff.is_none() {
                    RuntimeTypeParameterizedMatch::None
                } else {
                    RuntimeTypeParameterizedMatch::Nominal {
                        type_args: nominal.args.clone(),
                        effect_arg: nominal.eff.clone(),
                    }
                }
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => RuntimeTypeParameterizedMatch::Function {
                receiver: fun.receiver,
                params: fun.params.clone(),
                return_ty: fun.return_ty,
                effects: fun.effects.clone(),
                effects_closed: fun.effects_closed,
            },
            TypeKind::Ref(RefTypeKind::Union(union)) => RuntimeTypeParameterizedMatch::Union {
                variants: union.variants.clone(),
            },
            TypeKind::Value(ValueTypeKind::Option(payload_ty)) => {
                RuntimeTypeParameterizedMatch::Option {
                    payload_ty: *payload_ty,
                }
            }
            TypeKind::Value(ValueTypeKind::Tuple(element_tys)) => {
                RuntimeTypeParameterizedMatch::Tuple {
                    element_tys: element_tys.clone(),
                }
            }
            TypeKind::StarProjection(star) => RuntimeTypeParameterizedMatch::StarProjection {
                read_ty: star.read_ty,
            },
            TypeKind::Ref(RefTypeKind::Any)
            | TypeKind::Ref(RefTypeKind::String)
            | TypeKind::Value(_)
            | TypeKind::Param(_) => RuntimeTypeParameterizedMatch::None,
        }
    }

    pub(in crate::mir::lower) fn runtime_type_static_fold(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
    ) -> RuntimeTypeStaticFold {
        if source_ty == target_ty {
            return RuntimeTypeStaticFold::AlwaysTrue;
        }
        if target_ty == self.builtins.any {
            return RuntimeTypeStaticFold::AlwaysTrue;
        }
        if target_ty == self.builtins.nothing {
            return RuntimeTypeStaticFold::AlwaysFalse;
        }

        match (self.types.kind(source_ty), self.types.kind(target_ty)) {
            (TypeKind::Value(_), TypeKind::Value(_)) => RuntimeTypeStaticFold::AlwaysFalse,
            (TypeKind::Value(_), TypeKind::Ref(_)) => RuntimeTypeStaticFold::AlwaysFalse,
            (TypeKind::Ref(RefTypeKind::String), TypeKind::Value(_))
            | (TypeKind::Ref(RefTypeKind::Function(_)), TypeKind::Value(_))
            | (TypeKind::Ref(RefTypeKind::Union(_)), TypeKind::Value(_)) => {
                RuntimeTypeStaticFold::AlwaysFalse
            }
            (TypeKind::Ref(RefTypeKind::Nominal(nominal)), TypeKind::Value(_))
                if self.facts.nominal_kind(&nominal.fqn) != Some(ast::TypeKind::Interface) =>
            {
                RuntimeTypeStaticFold::AlwaysFalse
            }
            _ => RuntimeTypeStaticFold::Dynamic,
        }
    }

    pub(in crate::mir::lower) fn runtime_pattern_match_kind(
        &self,
        descriptor: &RuntimeTypeDescriptorKey,
        parameterized: &RuntimeTypeParameterizedMatch,
    ) -> RuntimePatternTypeTestKind {
        if !matches!(parameterized, RuntimeTypeParameterizedMatch::None) {
            return RuntimePatternTypeTestKind::RuntimeParameterized;
        }

        match &descriptor.kind {
            RuntimeTypeDescriptorKind::Nominal {
                kind: Some(ast::TypeKind::Class),
                ..
            } => RuntimePatternTypeTestKind::RuntimeClass,
            RuntimeTypeDescriptorKind::Nominal {
                kind: Some(ast::TypeKind::Interface),
                ..
            } => RuntimePatternTypeTestKind::RuntimeInterface,
            RuntimeTypeDescriptorKind::Nominal { .. } => RuntimePatternTypeTestKind::RuntimeNominal,
            RuntimeTypeDescriptorKind::Any
            | RuntimeTypeDescriptorKind::String
            | RuntimeTypeDescriptorKind::Function
            | RuntimeTypeDescriptorKind::Union => RuntimePatternTypeTestKind::RuntimeRef,
            RuntimeTypeDescriptorKind::Option
            | RuntimeTypeDescriptorKind::Tuple
            | RuntimeTypeDescriptorKind::Value
            | RuntimeTypeDescriptorKind::TypeParam
            | RuntimeTypeDescriptorKind::StarProjection => RuntimePatternTypeTestKind::StaticValue,
        }
    }

    pub(in crate::mir::lower) fn lower_short_circuit_binary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let lhs_local = self.lower_expr_to_local(lhs);
        if self.current_is_terminated() {
            return result;
        }

        let rhs_bb = self.push_block(rhs.span);
        let short_bb = self.push_block(span);
        let merge_bb = self.push_block(span);
        let parent = self.current_bb;

        let (then_target, else_target, short_value) = match op {
            ast::BinaryOp::LogAnd => (rhs_bb, short_bb, false),
            ast::BinaryOp::LogOr => (short_bb, rhs_bb, true),
            _ => unreachable!("caller guarantees short-circuit op"),
        };

        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(lhs_local),
                then_target,
                else_target,
            },
        );

        self.current_bb = short_bb;
        self.assign(
            span,
            result,
            Rvalue::Use(Operand::Const(ConstValue::Bool(short_value))),
        );
        self.set_terminator(short_bb, span, TerminatorKind::Goto { target: merge_bb });

        self.current_bb = rhs_bb;
        let rhs_local = self.lower_expr_to_local(rhs);
        if !self.current_is_terminated() {
            self.assign_use_to_local(span, result, Operand::Local(rhs_local));
            self.set_terminator(rhs_bb, span, TerminatorKind::Goto { target: merge_bb });
        }

        self.current_bb = merge_bb;
        result
    }

    pub(in crate::mir::lower) fn lower_type_check_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        value: &hir::Expr,
        op: ast::TypeCheckOp,
        test_ty: TypeId,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let value_local = self.lower_expr_to_local(value);
        if self.current_is_terminated() {
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::TypeCheck {
                value: Operand::Local(value_local),
                op,
                test_ty,
                metadata: self.runtime_type_test_metadata(value.ty, test_ty),
            },
        );
        result
    }

    pub(in crate::mir::lower) fn lower_cast_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        value: &hir::Expr,
        op: ast::CastOp,
        target_ty: TypeId,
    ) -> LocalId {
        let result_ty = if op == ast::CastOp::As { target_ty } else { ty };
        let result = self.push_temp_local(span, result_ty);
        let value_local = self.lower_expr_to_local(value);
        if self.current_is_terminated() {
            return result;
        }
        if op == ast::CastOp::As {
            self.lower_cast_as_expr_with_runtime_error_boundary(
                span,
                result,
                value,
                value_local,
                target_ty,
            );
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::Cast {
                value: Operand::Local(value_local),
                op,
                target_ty,
                metadata: self.runtime_cast_metadata(value.ty, target_ty, ty, op),
            },
        );
        result
    }

    pub(in crate::mir::lower) fn lower_cast_as_expr_with_runtime_error_boundary(
        &mut self,
        span: Span,
        result: LocalId,
        value: &hir::Expr,
        value_local: LocalId,
        target_ty: TypeId,
    ) {
        let mut metadata =
            self.runtime_cast_metadata(value.ty, target_ty, target_ty, ast::CastOp::As);
        let test_local = self.push_temp_local(span, self.builtins.bool_);
        self.assign(
            span,
            test_local,
            Rvalue::TypeCheck {
                value: Operand::Local(value_local),
                op: ast::TypeCheckOp::Is,
                test_ty: target_ty,
                metadata: metadata.test.clone(),
            },
        );

        let ok_bb = self.push_block(span);
        let fail_bb = self.push_block(span);
        let merge_bb = self.push_block(span);
        let parent = self.current_bb;
        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(test_local),
                then_target: ok_bb,
                else_target: fail_bb,
            },
        );

        self.current_bb = ok_bb;
        metadata.test.static_fold = RuntimeTypeStaticFold::AlwaysTrue;
        self.assign(
            span,
            result,
            Rvalue::Cast {
                value: Operand::Local(value_local),
                op: ast::CastOp::As,
                target_ty,
                metadata,
            },
        );
        self.set_terminator(ok_bb, span, TerminatorKind::Goto { target: merge_bb });

        self.current_bb = fail_bb;
        self.lower_cast_as_failure_raise(span, result, merge_bb);

        self.current_bb = merge_bb;
    }

    pub(in crate::mir::lower) fn lower_cast_as_failure_raise(
        &mut self,
        span: Span,
        result: LocalId,
        merge_bb: BasicBlockId,
    ) {
        let runtime_error_ty = find_runtime_error_type(self.types).unwrap_or(self.builtins.any);
        let effect_ty = find_raise_runtime_error_effect(self.types).unwrap_or(self.builtins.any);
        let error_local = self.push_temp_local(span, runtime_error_ty);
        self.assign(
            span,
            error_local,
            Rvalue::TopLevelRef(TopLevelRef {
                fqn: "scoop.core.RuntimeError.ClassCastFailed".to_string(),
                site_id: None,
                hidden_effects: EffectRow::pure(),
            }),
        );

        let perform_result = self.push_temp_local(span, self.builtins.nothing);
        self.assign(
            span,
            perform_result,
            Rvalue::PerformResult {
                op_fqn: "scoop.core.Raise.raise".to_string(),
                effect_ty,
            },
        );

        let resume_target = self.push_block(span);
        let site_id = self.fresh_site_id();
        let unwind = self.build_perform_unwind_action(span);
        let payload_transport = self.value_transport_with_boxing_reason(
            runtime_error_ty,
            MirTransportKind::EffectPayload,
            MirBoxingReason::EffectPayload,
            Some(runtime_error_ty),
        );
        self.set_terminator_with_unwind(
            self.current_bb,
            span,
            TerminatorKind::Perform {
                site_id,
                op_fqn: "scoop.core.Raise.raise".to_string(),
                metadata: PerformMetadata {
                    effect_ty,
                    op_type_args: Vec::new(),
                    result_ty: self.builtins.nothing,
                    payload_tuple_ty: Some(runtime_error_ty),
                    payload_component_tys: vec![runtime_error_ty],
                    payload_transport: vec![payload_transport],
                    arg_mapping: vec![0],
                },
                args: vec![PerformArg {
                    span,
                    source_arg_index: 0,
                    name: None,
                    value: Operand::Local(error_local),
                }],
                resume_target,
            },
            unwind,
        );

        self.current_bb = resume_target;
        self.assign_use_to_local(span, result, Operand::Local(perform_result));
        self.set_terminator(
            resume_target,
            span,
            TerminatorKind::Goto { target: merge_bb },
        );
    }

    pub(in crate::mir::lower) fn lower_member_access_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> LocalId {
        let tuple_member = self.tuple_member_access(member, receiver.ty);
        // HIR expr.ty 通常已经携带 smart-cast / branch narrowing 后的 authoritative 结果类型；
        // 但少量合成 HIR（例如 `with` copy builder）仍会先用 `Any` 占位，再依赖成员声明类型回填。
        let result_ty = tuple_member.map(|(_, elem_ty)| elem_ty).unwrap_or_else(|| {
            if ty == self.builtins.any {
                self.member_value_ty(member).unwrap_or(ty)
            } else {
                ty
            }
        });
        let result = self.push_temp_local(span, result_ty);
        let receiver_local = self.lower_expr_to_local(receiver);
        if self.current_is_terminated() {
            return result;
        }
        // smart-cast 之类的表达式语境可能把同一个 local 收窄到比声明更具体的类型；
        // 但合成 HIR（例如 extension property getter / `with` builder）也会临时把 receiver
        // 表达成宽的 `Any`。这里只在 expr.ty 比声明更具体时才建立视图 local，避免把已经
        // 正确的值类型 receiver 反向擦除成 `Any`。
        let receiver_local_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
        let receiver_local = if receiver.ty == self.builtins.any || receiver_local_ty == receiver.ty
        {
            receiver_local
        } else {
            let narrowed_receiver = self.push_temp_local(receiver.span, receiver.ty);
            self.assign_use_to_local(
                receiver.span,
                narrowed_receiver,
                Operand::Local(receiver_local),
            );
            narrowed_receiver
        };
        let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
        let tuple_member = tuple_member.or_else(|| self.tuple_member_access(member, receiver_ty));
        if let Some((index, _)) = tuple_member {
            self.assign(
                span,
                result,
                Rvalue::TupleGet {
                    tuple: Operand::Local(receiver_local),
                    index,
                },
            );
        } else {
            let member = self.lower_member_access_metadata(member, receiver_ty);
            let site_id = (!member.hidden_effects.is_pure()).then(|| self.fresh_site_id());
            self.assign(
                span,
                result,
                Rvalue::MemberAccess {
                    site_id,
                    receiver: Operand::Local(receiver_local),
                    member,
                },
            );
        }
        result
    }

    pub(in crate::mir::lower) fn member_value_ty(
        &self,
        member: &hir::MemberAccess,
    ) -> Option<TypeId> {
        let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
            return None;
        };
        self.facts.member_value_tys.get(fqn).copied()
    }

    pub(in crate::mir::lower) fn tuple_member_access(
        &self,
        member: &hir::MemberAccess,
        receiver_ty: TypeId,
    ) -> Option<(usize, TypeId)> {
        if member.resolved.is_some() {
            return None;
        }
        let index = parse_tuple_member_index(&member.name)?;
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(receiver_ty) else {
            return None;
        };
        elements.get(index).copied().map(|elem_ty| (index, elem_ty))
    }

    pub(in crate::mir::lower) fn lower_member_access_metadata(
        &self,
        member: &hir::MemberAccess,
        receiver_ty: TypeId,
    ) -> MemberAccessMetadata {
        let resolved = member.resolved.as_ref().map(|resolved| match resolved {
            hir::MemberRef::Value { fqn, .. } => MemberTarget::Value { fqn: fqn.clone() },
            hir::MemberRef::Fun { fqn, .. } => MemberTarget::Fun { fqn: fqn.clone() },
            hir::MemberRef::ExtensionValue { fqn, .. } => {
                MemberTarget::ExtensionValue { fqn: fqn.clone() }
            }
            hir::MemberRef::ExtensionFun { fqn, .. } => {
                MemberTarget::ExtensionFun { fqn: fqn.clone() }
            }
        });
        let hidden_effects = match &resolved {
            Some(MemberTarget::Value { fqn }) => self.facts.object_member_hidden_effects(fqn),
            _ => EffectRow::pure(),
        };
        MemberAccessMetadata {
            name: member.name.clone(),
            receiver_ty,
            resolved,
            hidden_effects,
        }
    }

    pub(in crate::mir::lower) fn lower_call_args(
        &mut self,
        args: &[hir::CallArg],
    ) -> Option<Vec<CallArg>> {
        self.lower_call_args_with_expected(args, &[])
    }

    pub(in crate::mir::lower) fn lower_call_args_with_expected(
        &mut self,
        args: &[hir::CallArg],
        expected_tys: &[Option<TypeId>],
    ) -> Option<Vec<CallArg>> {
        let mut out = Vec::with_capacity(args.len());
        for arg in args {
            if self.current_is_terminated() {
                return None;
            }
            let arg_index = out.len();
            match arg {
                hir::CallArg::Positional(expr) => {
                    if let Some(target_ty) = expected_tys.get(arg_index).and_then(|ty| *ty)
                        && let Some(operand) = self.scalar_literal_call_arg_operand(expr, target_ty)
                    {
                        out.push(CallArg {
                            span: expr.span,
                            name: None,
                            value: operand,
                        });
                        continue;
                    }
                    let value = self.lower_expr_to_local(expr);
                    if self.current_is_terminated() {
                        return None;
                    }
                    let operand = expected_tys
                        .get(arg_index)
                        .and_then(|ty| *ty)
                        .map(|target_ty| {
                            self.operand_for_target_ty(expr.span, Operand::Local(value), target_ty)
                        })
                        .unwrap_or(Operand::Local(value));
                    out.push(CallArg {
                        span: expr.span,
                        name: None,
                        value: operand,
                    });
                }
                hir::CallArg::Named { name, value, .. } => {
                    if let Some(target_ty) = expected_tys.get(arg_index).and_then(|ty| *ty)
                        && let Some(operand) =
                            self.scalar_literal_call_arg_operand(value, target_ty)
                    {
                        out.push(CallArg {
                            span: value.span,
                            name: Some(name.clone()),
                            value: operand,
                        });
                        continue;
                    }
                    let operand_local = self.lower_expr_to_local(value);
                    if self.current_is_terminated() {
                        return None;
                    }
                    let operand = expected_tys
                        .get(arg_index)
                        .and_then(|ty| *ty)
                        .map(|target_ty| {
                            self.operand_for_target_ty(
                                value.span,
                                Operand::Local(operand_local),
                                target_ty,
                            )
                        })
                        .unwrap_or(Operand::Local(operand_local));
                    out.push(CallArg {
                        span: value.span,
                        name: Some(name.clone()),
                        value: operand,
                    });
                }
            }
        }
        Some(out)
    }

    fn scalar_literal_call_arg_operand(
        &mut self,
        expr: &hir::Expr,
        target_ty: TypeId,
    ) -> Option<Operand> {
        let const_value = match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Bool(value)) => ConstValue::Bool(*value),
            hir::ExprKind::Literal(hir::LiteralKind::Char(_)) => ConstValue::Char,
            hir::ExprKind::Literal(hir::LiteralKind::Unit) => ConstValue::Unit,
            hir::ExprKind::Literal(hir::LiteralKind::Int) => ConstValue::Int,
            hir::ExprKind::Literal(hir::LiteralKind::SynthInt(value)) => {
                ConstValue::SynthInt(*value)
            }
            hir::ExprKind::Literal(hir::LiteralKind::Float64(_)) => ConstValue::Float64,
            hir::ExprKind::Literal(hir::LiteralKind::Float32(_)) => ConstValue::Float32,
            _ => return None,
        };
        Some(self.operand_for_target_ty(expr.span, Operand::Const(const_value), target_ty))
    }

    /// 将 HIR side table 发布的 call-arg binding 收口为稳定的 MIR 槽位顺序。
    ///
    /// 这里仅处理已显式 contract 化的简单 receiver/explicit case；
    /// 对 default/vararg/spread 等更复杂形状维持原顺序，避免在 MIR lowering 现场猜测。
    pub(in crate::mir::lower) fn canonicalize_call_args_from_binding(
        &self,
        args: Vec<CallArg>,
        binding: Option<&CallArgBindingContract>,
    ) -> Vec<CallArg> {
        let Some(binding) = binding else {
            return args;
        };

        let mut claimed_source_args = vec![false; args.len()];
        let mut ordered_source_indices = Vec::with_capacity(binding.params().len());
        let mut receiver_slot: Option<usize> = None;

        for (param_idx, param) in binding.params().iter().enumerate() {
            match param {
                CallArgParamContract::Explicit(element) => {
                    if element.spread() {
                        return args;
                    }
                    let source_arg_idx = element.arg_index();
                    if source_arg_idx >= args.len() || claimed_source_args[source_arg_idx] {
                        return args;
                    }
                    claimed_source_args[source_arg_idx] = true;
                    ordered_source_indices.push(source_arg_idx);
                }
                CallArgParamContract::Receiver => {
                    if receiver_slot.replace(param_idx).is_some() {
                        return args;
                    }
                    ordered_source_indices.push(usize::MAX);
                }
                CallArgParamContract::Default | CallArgParamContract::Vararg(_) => {
                    return args;
                }
            }
        }

        if ordered_source_indices.len() != args.len() {
            return args;
        }

        let receiver_source_arg_idx = if receiver_slot.is_some() {
            let mut unclaimed = claimed_source_args
                .iter()
                .enumerate()
                .filter_map(|(idx, claimed)| (!*claimed).then_some(idx));
            let Some(receiver_source_arg_idx) = unclaimed.next() else {
                return args;
            };
            if unclaimed.next().is_some() {
                return args;
            }
            Some(receiver_source_arg_idx)
        } else {
            if claimed_source_args.iter().any(|claimed| !*claimed) {
                return args;
            }
            None
        };

        let mut ordered = Vec::with_capacity(args.len());
        for source_arg_idx in ordered_source_indices {
            let source_arg_idx = if source_arg_idx == usize::MAX {
                receiver_source_arg_idx
                    .expect("receiver slot should exist when placeholder is used")
            } else {
                source_arg_idx
            };
            let mut arg = args[source_arg_idx].clone();
            arg.name = None;
            ordered.push(arg);
        }
        ordered
    }

    pub(in crate::mir::lower) fn hir_call_args_are_already_canonical(
        args: &[hir::CallArg],
    ) -> bool {
        args.iter()
            .all(|arg| matches!(arg, hir::CallArg::Positional(_)))
    }

    pub(in crate::mir::lower) fn active_hir_call_arg_binding<'b>(
        args: &[hir::CallArg],
        binding: Option<&'b CallArgBindingContract>,
    ) -> Option<&'b CallArgBindingContract> {
        // HIR canonical call lowering has already turned named/default/receiver surfaces into
        // ordered positional args while preserving source evaluation order with temporaries.
        // MIR must not apply the same binding a second time, or those args get shuffled back.
        if Self::hir_call_args_are_already_canonical(args) {
            None
        } else {
            binding
        }
    }

    pub(in crate::mir::lower) fn source_arg_expected_tys_from_param_tys(
        &self,
        param_tys: &[TypeId],
        explicit_arg_count: usize,
        args_include_receiver: bool,
        binding: Option<&CallArgBindingContract>,
    ) -> Vec<Option<TypeId>> {
        let mut expected = vec![None; explicit_arg_count];
        if let Some(binding) = binding {
            if args_include_receiver && call_arg_binding_has_receiver(binding) {
                return expected;
            }
            self.fill_expected_tys_from_arg_binding(&mut expected, param_tys, binding);
            return expected;
        }
        for (index, target_ty) in param_tys.iter().copied().enumerate().take(expected.len()) {
            expected[index] = Some(target_ty);
        }
        expected
    }

    pub(in crate::mir::lower) fn fill_expected_tys_from_arg_binding(
        &self,
        expected: &mut [Option<TypeId>],
        param_tys: &[TypeId],
        binding: &CallArgBindingContract,
    ) {
        let mut claimed_source_args = vec![false; expected.len()];
        let mut receiver_target_ty = None;
        for (param_index, param) in binding.params().iter().enumerate() {
            let Some(target_ty) = param_tys.get(param_index).copied() else {
                continue;
            };
            match param {
                CallArgParamContract::Receiver => receiver_target_ty = Some(target_ty),
                CallArgParamContract::Explicit(element) => {
                    if let Some(slot) = expected.get_mut(element.arg_index()) {
                        *slot = Some(target_ty);
                        claimed_source_args[element.arg_index()] = true;
                    }
                }
                CallArgParamContract::Vararg(elements) => {
                    for element in elements {
                        if let Some(slot) = expected.get_mut(element.arg_index()) {
                            *slot = Some(target_ty);
                            claimed_source_args[element.arg_index()] = true;
                        }
                    }
                }
                CallArgParamContract::Default => {}
            }
        }
        if let Some(target_ty) = receiver_target_ty {
            let mut unclaimed = claimed_source_args
                .iter()
                .enumerate()
                .filter_map(|(idx, claimed)| (!*claimed).then_some(idx));
            if let Some(receiver_idx) = unclaimed.next()
                && unclaimed.next().is_none()
                && let Some(slot) = expected.get_mut(receiver_idx)
            {
                *slot = Some(target_ty);
            }
        }
    }
}
