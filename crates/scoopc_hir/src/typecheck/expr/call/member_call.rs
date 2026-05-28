//! Member-call type inference (single large entry point).

#![allow(dead_code)]

use super::*;

pub(super) fn infer_member_call_expr_type(
    inputs: ExprInferInputs<'_>,
    request: MemberCallRequest<'_>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let MemberCallRequest {
        call_expr,
        receiver,
        member,
        args,
        explicit_type_args,
        explicit_eff_arg,
        safe,
    } = request;
    let source = inputs.source;
    let builtins = inputs.builtins;
    let locals = inputs.locals;
    let top_level_types = inputs.top_level_types;
    let top_level_funs = inputs.top_level_funs;
    let struct_field_types = inputs.struct_field_types;

    // 先递归类型检查 receiver：保证 `a?.b()` 中的 `a` 自身也会被覆盖。
    //
    // 例外：`TypeName.member(...)` 的 companion dispatch 中，receiver 是一个类型名而不是值表达式；
    // resolver 会刻意保留该 ident 为未解析状态，而实际运行期 receiver 应是 companion object 单例值。
    // 这里直接把 receiver 视为 companion object 的名义类型，避免把 `TypeName` 当普通值去推导。
    let companion_receiver_owner_fqn = if let ast::ExprKind::Ident(id) = &receiver.kind
        && id.resolved.is_none()
        && source.slice(id.span) != "this"
    {
        match member.resolved.as_ref() {
            Some(ast::ResolvedMemberRef::Fun { fqn })
            | Some(ast::ResolvedMemberRef::Value { fqn }) => {
                if let Some((owner_fqn, _)) = fqn.rsplit_once('.') {
                    lower
                        .is_object_type(owner_fqn)
                        .then(|| owner_fqn.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    };
    let receiver_ty = if let Some(owner_fqn) = companion_receiver_owner_fqn {
        lower.lower_type_fqn_with_args(owner_fqn, Vec::new(), receiver.span)?
    } else {
        inputs.infer(lower, receiver)?
    };

    let actual_receiver_ty = if safe {
        match lower.type_kind(receiver_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
            _ => {
                return Err(ExprTypeError::SafeAccessReceiverNotNullable {
                    found: lower.fmt_type(receiver_ty),
                    span: receiver.span.into(),
                });
            }
        }
    } else {
        receiver_ty
    };

    if let Some(ret) = try_infer_continuation_resume_call_expr_type(
        inputs,
        call_expr,
        actual_receiver_ty,
        member,
        args,
        safe,
        lower,
    )? {
        return Ok(ret);
    }

    // String byte-level substrate.
    //
    // Public helpers such as `length/toInt/concat/hash/isEmpty/replace/charAt/repeat/compareTo/trimIndent/unsafeSliceBytes`
    // are ordinary `String` body methods now.  Only byte-level physical-layout access remains synthetic.
    let member_name = source.slice(member.span);
    if actual_receiver_ty == builtins.string && matches!(member_name, "byteLength" | "getByte") {
        let callee_fqn = format!("scoop.core.{member_name}");
        let call_args = collect_call_arg_infos(inputs, args, lower)?;
        check_call_arg_named_rules(&callee_fqn, &call_args)?;
        let (param_names, param_tys, return_ty) = match member_name {
            "byteLength" => (Vec::new(), Vec::new(), builtins.int),
            "getByte" => (vec!["index".to_string()], vec![builtins.int], builtins.int),
            _ => unreachable!("filtered by matches!"),
        };
        check_call_named_args_exist_in_any_candidate(
            &callee_fqn,
            &call_args,
            std::iter::once(param_names.as_slice()),
        )?;
        if call_args.len() != param_names.len() {
            return Err(ExprTypeError::CallArityMismatch {
                callee: member_name.into(),
                expected: param_names.len(),
                found: call_args.len(),
                span: call_expr.span.into(),
            });
        }
        let param_has_defaults = vec![false; param_names.len()];
        let Some(mapping) =
            map_call_args_to_params_with_defaults(&call_args, &param_names, &param_has_defaults)
        else {
            return Err(ExprTypeError::NoMatchingOverload {
                callee: callee_fqn.clone(),
                span: call_expr.span.into(),
            });
        };
        for (param_idx, expected_ty) in param_tys.iter().copied().enumerate() {
            let Some(arg_idx) = mapping.get(param_idx).copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: member_name.into(),
                    expected: param_names.len(),
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };
            let arg = &call_args[arg_idx];
            if !is_type_assignable(arg.ty, expected_ty, lower, builtins)
                && !literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins)
            {
                return Err(ExprTypeError::CallArgTypeMismatch {
                    callee: callee_fqn.clone(),
                    index: param_idx + 1,
                    expected: lower.fmt_type(expected_ty),
                    found: lower.fmt_type(arg.ty),
                    span: arg.expr.span.into(),
                });
            }
        }
        record_receiver_prefixed_extension_call_binding(
            lower,
            call_expr.span,
            member.span,
            &callee_fqn,
            &mapping,
            &call_args,
        );
        return Ok(return_ty);
    }

    let current_lambda_this = inputs.is_current_lambda_this_expr(receiver);
    let force_late_direct_member =
        actual_receiver_ty == builtins.string && matches!(member_name, "hash" | "toInt");
    let pre_resolved_extension_fun = matches!(
        member.resolved.as_ref(),
        Some(ast::ResolvedMemberRef::ExtensionFun { .. })
    );
    // Direct receiver members take precedence over extension functions once the receiver type is known.
    let late_direct_member_fun_fqn = if current_lambda_this
        || member.resolved.is_none()
        || pre_resolved_extension_fun
        || force_late_direct_member
    {
        late_resolve_direct_member_fun_fqn_from_receiver_ty(
            inputs,
            actual_receiver_ty,
            member_name,
            lower,
        )?
    } else {
        None
    };
    let resolved_member_fun_fqn = late_direct_member_fun_fqn.as_deref().or({
        if current_lambda_this {
            None
        } else {
            match member.resolved.as_ref() {
                Some(ast::ResolvedMemberRef::Fun { fqn }) => Some(fqn.as_str()),
                _ => None,
            }
        }
    });
    if let Some(fqn) = resolved_member_fun_fqn {
        lower.record_typechecked_member_resolution(
            member.span,
            ast::ResolvedMemberRef::Fun {
                fqn: fqn.to_string(),
            },
        );
    }

    // spec §15.10 / §15.10.1：GC pin/handle intrinsic surface。
    //
    // 说明：
    // - `GC.pin/unpin` 与 `GC.handleNew/Get/Drop` 是 sysroot 固定的 intrinsic member-call surface；
    //   它们的 authoritative contract 由前端 gate、MIR transport metadata 与 runtime lowering 共同定义。
    // - 这里保留专门分支，是为了在 ordinary member-call desugaring 之前锁定支持面与诊断，避免把它们
    //   误降成普通成员调用后再由后端兜底。
    // - `pin/handleNew` 只接受可追踪引用对象；`unpin`/`handleGet`/`handleDrop` 只接受对应 token 类型。
    if let Some(fqn) = resolved_member_fun_fqn {
        // `handleNew` 可能分配，因此在 `@NoGC` 上下文中必须拒绝；其余入口沿 sysroot `@NoGC`/
        // `@Unsafe` contract 执行。
        if fqn == "scoop.core.GC.handleNew" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }
            if lower.in_nogc_context() {
                return Err(ExprTypeError::NoGcCallForbidden {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["obj".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let obj_ty = call_args[arg_idx].ty;
            if !matches!(lower.type_kind(obj_ty), TypeKind::Ref(_)) {
                return Err(ExprTypeError::GcHandleNewRequiresRefType {
                    found: lower.fmt_type(obj_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            let handle_ty = lower.lower_type_fqn_with_args(
                "scoop.core.GcHandle".to_string(),
                Vec::new(),
                call_expr.span,
            )?;
            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(handle_ty);
        }

        if fqn == "scoop.core.GC.handleGet" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["h".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let handle_ty = call_args[arg_idx].ty;
            let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(handle_ty)
            else {
                return Err(ExprTypeError::GcHandleGetRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            };
            if nominal.fqn != "scoop.core.GcHandle" || !nominal.args.is_empty() {
                return Err(ExprTypeError::GcHandleGetRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(builtins.any);
        }

        if fqn == "scoop.core.GC.handleDrop" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["h".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let handle_ty = call_args[arg_idx].ty;
            let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(handle_ty)
            else {
                return Err(ExprTypeError::GcHandleDropRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            };
            if nominal.fqn != "scoop.core.GcHandle" || !nominal.args.is_empty() {
                return Err(ExprTypeError::GcHandleDropRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(builtins.unit);
        }

        if fqn == "scoop.core.GC.pin" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["obj".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let obj_ty = call_args[arg_idx].ty;
            if !matches!(lower.type_kind(obj_ty), TypeKind::Ref(_)) {
                return Err(ExprTypeError::GcPinRequiresRefType {
                    found: lower.fmt_type(obj_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            let pinned_ty = lower.lower_type_fqn_with_args(
                "scoop.core.Pinned".to_string(),
                Vec::new(),
                call_expr.span,
            )?;
            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(pinned_ty);
        }

        if fqn == "scoop.core.GC.unpin" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["pinned".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let pinned_ty = call_args[arg_idx].ty;
            let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(pinned_ty)
            else {
                return Err(ExprTypeError::GcUnpinRequiresRefType {
                    found: lower.fmt_type(pinned_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            };
            if nominal.fqn != "scoop.core.Pinned" || !nominal.args.is_empty() {
                return Err(ExprTypeError::GcUnpinRequiresRefType {
                    found: lower.fmt_type(pinned_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(builtins.unit);
        }
    }

    // T1508a：直连成员函数调用（final/private）。
    //
    // 说明：
    // - resolver 在 member access 阶段只做"存在性 + FQN 写回"，不会为 member fun call 收集 overload set；
    // - 这里把 `receiver.method(args...)` 降到"对 FQN overload set 的普通调用"来做重载决议，
    //   并把 `receiver` 作为隐式第 0 个参数参与类型检查；
    // - 当前入口统一覆盖 direct call / class vtable / interface itable 三类成员调用形态；
    //   具体走哪条后端路径由 receiver 类型与 slot 解析结果决定。
    if let Some(fqn) = resolved_member_fun_fqn {
        // 注意：`GC.pin/unpin` / `GC.handle*` 走的是专门的 GC intrinsic contract；这里不要把它们当作普通 member call。
        if fqn != "scoop.core.GC.pin"
            && fqn != "scoop.core.GC.unpin"
            && fqn != "scoop.core.GC.handleNew"
            && fqn != "scoop.core.GC.handleGet"
            && fqn != "scoop.core.GC.handleDrop"
            // T0130 修复：当 receiver 为 TypeKind::Param 时，跳过直连成员调用路径，
            // 让后续 where-bound 驱动的方法分发来处理（否则 try_extract_nominal_fqn_and_args
            // 会因 Param 非 nominal 而返回 CalleeNotCallable）。
            && !matches!(lower.type_kind(actual_receiver_ty), TypeKind::Param(_))
        {
            // P4-T01l：让 builtin scalar / `String` receiver 也能在 direct-call 主线里
            // 落入 nominal member-call FQN，从而把 receiver 作为第 0 个 arg 注入。
            let Some((receiver_fqn, receiver_args)) =
                try_extract_member_call_receiver_fqn_and_args(actual_receiver_ty, lower)
            else {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: fqn.to_string(),
                    span: member.span.into(),
                });
            };

            let sigs = collect_member_method_signatures_from_index(
                source,
                actual_receiver_ty,
                &receiver_fqn,
                &receiver_args,
                fqn,
                lower,
                builtins,
            )?;
            if sigs.is_empty() {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: fqn.to_string(),
                    span: member.span.into(),
                });
            }

            // 预先推导所有"显式实参"的类型（不含 receiver），并归一化 named arg 的语法糖节点，
            // 以便在重载筛选中复用这份结果并避免把子表达式错误吞掉。
            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            let synthesized_unit_args = args
                .is_empty()
                .then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
            let sugar_call_args = if let Some(synthesized_args) = synthesized_unit_args.as_ref() {
                Some(collect_call_arg_infos(inputs, synthesized_args, lower)?)
            } else {
                None
            };
            check_call_arg_named_rules(fqn, &call_args)?;
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                sigs.iter().filter_map(|s| s.param_names.get(1..)),
            )?;

            let receiver_arg = CallArgInfo {
                kind: CallArgKind::Positional,
                expr: receiver,
                ty: actual_receiver_ty,
                is_spread: false,
                needs_expected_type: false,
            };

            let mut call_args_with_receiver = Vec::with_capacity(call_args.len() + 1);
            call_args_with_receiver.push(receiver_arg.clone());
            call_args_with_receiver.extend(call_args.iter().cloned());

            #[derive(Debug, Clone)]
            struct MatchedMemberOverload<'a> {
                sig: &'a FunSigOwned,
                instantiated: InstantiatedFunSig,
                eff_arg: EffectRow,
                /// `call_args_with_receiver[arg_idx]` 对应的"期望类型"。
                expected_arg_tys: Vec<TypeId>,
                /// 调用点需要用默认值补齐的形参个数（越少越"具体"）。
                defaults_used: usize,
                /// 形参 -> 实参绑定（用于后续门禁，例如 `addressOf(var: T)`）。
                mapping: Vec<ParamArgBinding>,
                /// 当前候选是否通过 typed `Unit` zero-arg sugar 匹配得到。
                used_unit_sugar: bool,
            }

            fn is_strictly_more_specific_member_overload(
                a: &MatchedMemberOverload<'_>,
                b: &MatchedMemberOverload<'_>,
                lower: &TypeLowering<'_>,
                builtins: BuiltinTypes,
            ) -> bool {
                let a_le_b = a
                    .expected_arg_tys
                    .iter()
                    .zip(b.expected_arg_tys.iter())
                    .all(|(a_ty, b_ty)| is_type_assignable(*a_ty, *b_ty, lower, builtins));
                let b_le_a = b
                    .expected_arg_tys
                    .iter()
                    .zip(a.expected_arg_tys.iter())
                    .all(|(b_ty, a_ty)| is_type_assignable(*b_ty, *a_ty, lower, builtins));

                a_le_b && !b_le_a
            }

            fn pick_most_specific_member_overload(
                candidates: &[MatchedMemberOverload<'_>],
                lower: &TypeLowering<'_>,
                builtins: BuiltinTypes,
            ) -> Option<usize> {
                // 1) Kotlin-like most-specific：候选 A 的每个形参类型都"更具体"（可赋值到 B 的形参类型），
                //    且至少有一个位置严格更具体，则认为 A 严格更具体。
                for (idx, cand) in candidates.iter().enumerate() {
                    let mut ok = true;
                    for (other_idx, other) in candidates.iter().enumerate() {
                        if idx == other_idx {
                            continue;
                        }
                        if !is_strictly_more_specific_member_overload(cand, other, lower, builtins)
                        {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        return Some(idx);
                    }
                }

                // 2) tie-break：默认参数更少者优先（"非默认参数优先"）。
                let min_defaults = candidates
                    .iter()
                    .map(|c| c.defaults_used)
                    .min()
                    .unwrap_or(0);
                let mut it = candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.defaults_used == min_defaults);
                let (idx, _) = it.next()?;
                if it.next().is_some() {
                    return None;
                }
                Some(idx)
            }

            let mut matched: Vec<MatchedMemberOverload<'_>> = Vec::new();
            for cand in sigs.iter() {
                let Some((user_param_tys, param_has_defaults, param_is_vararg)) =
                    user_visible_param_slices_after_receiver(
                        &cand.params,
                        &cand.param_has_defaults,
                        &cand.param_is_vararg,
                    )
                else {
                    continue;
                };
                let exact_mapping = map_call_args_to_params_with_defaults_and_varargs(
                    &call_args_with_receiver,
                    &cand.param_names,
                    &cand.param_has_defaults,
                    &cand.param_is_vararg,
                );
                let (call_args_for_candidate, mapping, used_unit_sugar) =
                    if let Some(mapping) = exact_mapping {
                        (call_args_with_receiver.clone(), mapping, false)
                    } else if can_use_zero_arg_unit_call_sugar(
                        args,
                        user_param_tys,
                        param_has_defaults,
                        param_is_vararg,
                        lower,
                    ) {
                        let Some(sugar_call_args) = sugar_call_args.as_ref() else {
                            continue;
                        };
                        let mut sugar_call_args_with_receiver =
                            Vec::with_capacity(sugar_call_args.len() + 1);
                        sugar_call_args_with_receiver.push(receiver_arg.clone());
                        sugar_call_args_with_receiver.extend(sugar_call_args.iter().cloned());
                        let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                            &sugar_call_args_with_receiver,
                            &cand.param_names,
                            &cand.param_has_defaults,
                            &cand.param_is_vararg,
                        ) else {
                            continue;
                        };
                        (sugar_call_args_with_receiver, mapping, true)
                    } else {
                        continue;
                    };

                // spread 实参只能绑定到 vararg 形参；否则该候选不匹配。
                let mut ok = true;
                for binding in mapping.iter() {
                    match binding {
                        ParamArgBinding::Default => {}
                        ParamArgBinding::Single(arg_idx) => {
                            if call_args_for_candidate
                                .get(*arg_idx)
                                .is_some_and(|a| a.is_spread)
                            {
                                ok = false;
                                break;
                            }
                        }
                        ParamArgBinding::Vararg(_) => {}
                    }
                }
                if !ok {
                    continue;
                }

                let mapping_pairs = expand_param_arg_pairs(&mapping);

                let mut generic_constraints: Vec<GenericArgConstraint> =
                    Vec::with_capacity(mapping_pairs.len());
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args_for_candidate[arg_idx];
                    if arg.is_spread {
                        if !cand
                            .param_is_vararg
                            .get(param_idx)
                            .copied()
                            .unwrap_or(false)
                        {
                            ok = false;
                            break;
                        }
                        let Some(elem_tys) = spread_operand_element_types(arg.ty, lower) else {
                            ok = false;
                            break;
                        };
                        for found_elem in elem_tys {
                            generic_constraints.push(GenericArgConstraint {
                                expected: cand.params[param_idx],
                                found: found_elem,
                                found_is_placeholder: false,
                                from: format!("第 {} 个实参（spread）", arg_idx + 1),
                                span: arg.expr.span,
                            });
                        }
                        continue;
                    }

                    generic_constraints.push(GenericArgConstraint {
                        expected: cand.params[param_idx],
                        found: arg.ty,
                        found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                        from: format!("第 {} 个实参", arg_idx + 1),
                        span: arg.expr.span,
                    });
                }
                if !ok {
                    continue;
                }

                let mut instantiated =
                    match instantiate_fun_sig_for_call_with_optional_explicit_type_args(
                        fqn,
                        call_expr.span,
                        cand,
                        explicit_type_args,
                        generic_constraints,
                        lower,
                        builtins,
                    ) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                // T0129：检查 where 约束；不满足则跳过该候选。
                if check_fun_where_constraints_after_instantiation(
                    fqn,
                    call_expr.span,
                    cand,
                    &instantiated.type_args,
                    lower,
                    builtins,
                )
                .is_err()
                {
                    continue;
                }

                // 只在需要时（lambda）进入 expected-context typecheck，避免在候选尝试期间把"候选相关"的
                // 副作用（例如调用 required effects）写进外层函数体的 effects 集合。
                let mut checked_arg_tys: Vec<TypeId> =
                    call_args_for_candidate.iter().map(|a| a.ty).collect();
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args_for_candidate[arg_idx];
                    if arg.is_spread {
                        continue;
                    }
                    if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                        continue;
                    }

                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = match inputs.infer_in_expected(
                        lower,
                        arg.expr,
                        expected_ty,
                        ExpectedTypeFrom::new(format!(
                            "`{}` 的第 {} 个形参 `{}`",
                            fqn,
                            param_idx + 1,
                            cand.param_names[param_idx]
                        )),
                    ) {
                        Ok(ty) => ty,
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    };
                    checked_arg_tys[arg_idx] = found_ty;
                }
                if !ok {
                    continue;
                }

                let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.cloned() {
                    explicit_eff_arg
                } else if let Some(eff_param) = &cand.eff_param {
                    let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        let arg = &call_args_for_candidate[arg_idx];
                        if arg.is_spread {
                            continue;
                        }

                        if let Some(base) = cand
                            .param_nominal_eff_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        {
                            let base = match substitute_type_args_in_effect_row(
                                base.clone(),
                                &cand.type_params,
                                &instantiated.type_args,
                                lower,
                                call_expr.span,
                            ) {
                                Ok(row) => row,
                                Err(_) => continue,
                            };
                            let found_ty = checked_arg_tys[arg_idx];
                            if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                                let delta = effect_row_difference(&found_row, &base);
                                terms.extend(delta.terms);
                            }
                        }

                        let Some(base) = cand
                            .param_fn_effect_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };
                        if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                            continue;
                        }

                        let base = match substitute_type_args_in_effect_row(
                            base.clone(),
                            &cand.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        ) {
                            Ok(row) => row,
                            Err(_) => continue,
                        };
                        let found_ty = checked_arg_tys[arg_idx];
                        if let TypeKind::Ref(RefTypeKind::Function(found_fun)) =
                            lower.type_kind(found_ty)
                        {
                            let delta = effect_row_difference(&found_fun.effects, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    let inferred = EffectRow::new(terms);
                    match substitute_type_args_in_effect_row(
                        inferred,
                        &cand.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    ) {
                        Ok(row) => row,
                        Err(_) => continue,
                    }
                } else {
                    EffectRow::pure()
                };

                if cand.eff_param.is_some()
                    && instantiate_eff_row_var_in_sig_types(
                        cand,
                        &mut instantiated,
                        &eff_arg,
                        lower,
                        call_expr.span,
                    )
                    .is_err()
                {
                    continue;
                }

                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args_for_candidate[arg_idx];
                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = checked_arg_tys[arg_idx];

                    if arg.is_spread {
                        if !cand
                            .param_is_vararg
                            .get(param_idx)
                            .copied()
                            .unwrap_or(false)
                        {
                            ok = false;
                            break;
                        }
                        let Some(elem_tys) = spread_operand_element_types(found_ty, lower) else {
                            ok = false;
                            break;
                        };
                        for elem_ty in elem_tys {
                            if is_type_assignable(elem_ty, expected_ty, lower, builtins) {
                                continue;
                            }
                            ok = false;
                            break;
                        }
                        if !ok {
                            break;
                        }
                        continue;
                    }

                    if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                        continue;
                    }
                    if literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins) {
                        continue;
                    }
                    ok = false;
                    break;
                }
                if !ok {
                    continue;
                }

                let defaults_used = mapping
                    .iter()
                    .filter(|b| matches!(b, ParamArgBinding::Default))
                    .count();
                let mut expected_arg_tys = vec![builtins.nothing; call_args_for_candidate.len()];
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    expected_arg_tys[arg_idx] = instantiated.params[param_idx];
                }

                matched.push(MatchedMemberOverload {
                    sig: cand,
                    instantiated,
                    eff_arg,
                    expected_arg_tys,
                    defaults_used,
                    mapping,
                    used_unit_sugar,
                });
            }

            if matched.iter().any(|cand| !cand.used_unit_sugar) {
                matched.retain(|cand| !cand.used_unit_sugar);
            }

            let chosen = match matched.len() {
                0 => {
                    let name = short_name_from_fqn(fqn).to_string();
                    let candidates = join_overload_rejections(
                        sigs.iter()
                            .map(|cand| OverloadRejection {
                                signature: fmt_overload_signature(&name, None, &cand.params, lower),
                                location: format_candidate_location(
                                    lower,
                                    &cand.decl_file,
                                    cand.decl_span,
                                ),
                                reason: describe_basic_applicability_rejection(
                                    BasicApplicabilityRejection {
                                        call_args: &call_args_with_receiver,
                                        param_names: &cand.param_names,
                                        param_has_defaults: &cand.param_has_defaults,
                                        param_is_vararg: &cand.param_is_vararg,
                                        param_tys: &cand.params,
                                        source,
                                        lower,
                                        builtins,
                                    },
                                ),
                            })
                            .collect(),
                    );
                    return Err(ExprTypeError::NoApplicableOverload {
                        callee: fqn.to_string(),
                        candidates,
                        span: call_expr.span.into(),
                    });
                }
                1 => matched.pop().expect("len == 1"),
                _ => {
                    let Some(idx) = pick_most_specific_member_overload(&matched, lower, builtins)
                    else {
                        let name = short_name_from_fqn(fqn).to_string();
                        let candidates = join_overload_signatures(
                            matched
                                .iter()
                                .map(|c| {
                                    fmt_overload_signature(
                                        &name,
                                        None,
                                        &c.instantiated.params,
                                        lower,
                                    )
                                })
                                .collect(),
                        );
                        return Err(ExprTypeError::AmbiguousOverload {
                            callee: fqn.to_string(),
                            candidates,
                            span: call_expr.span.into(),
                        });
                    };
                    matched.swap_remove(idx)
                }
            };

            check_unsafe_call_gate(fqn, chosen.sig, call_expr.span, lower)?;
            check_nogc_call_gate(fqn, chosen.sig, call_expr.span, lower)?;
            emit_deprecated_call_warning(fqn, chosen.sig, call_expr.span, lower);
            let chosen_call_args = if chosen.used_unit_sugar {
                let sugar_call_args = sugar_call_args
                    .as_ref()
                    .expect("typed Unit sugar 选择的成员调用应有合成实参");
                let mut chosen_call_args = Vec::with_capacity(sugar_call_args.len() + 1);
                chosen_call_args.push(receiver_arg.clone());
                chosen_call_args.extend(sugar_call_args.iter().cloned());
                chosen_call_args
            } else {
                call_args_with_receiver.clone()
            };
            check_var_param_lvalue_gate(fqn, chosen.sig, &chosen_call_args, &chosen.mapping)?;

            // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
            let type_param_bindings = type_param_bindings_from_sig(&chosen.sig.type_params, lower);
            let eff_bindings: Vec<(String, EffectRow)> = chosen
                .sig
                .eff_param
                .as_ref()
                .map(|p| vec![(p.name.clone(), chosen.eff_arg.clone())])
                .unwrap_or_default();
            let lowered_effects = lower.lower_effect_row_expr_in_decl_file_with_scopes(
                &chosen.sig.decl_file,
                type_param_bindings,
                eff_bindings,
                chosen.sig.effects.as_ref(),
            );
            let call_effects = substitute_type_args_in_effect_row(
                lowered_effects?,
                &chosen.sig.type_params,
                &chosen.instantiated.type_args,
                lower,
                call_expr.span,
            )?;
            for effect in call_effects.terms.iter().copied() {
                lower.record_performed_effect(effect, call_expr.span);
            }

            // T0712/T5000e2b：记录带 receiver 的 direct-call 实例请求。
            // 对 generic owner member/getter，这里需要把 owner-specialization 的 concrete args
            // 放在函数自身 type args 之前，形成可复用的实例身份。
            let eff_args = chosen
                .sig
                .eff_param
                .as_ref()
                .map(|_| vec![chosen.eff_arg.clone()])
                .unwrap_or_default();
            let type_args = combined_member_instance_type_args(
                fqn,
                actual_receiver_ty,
                &chosen.instantiated.type_args,
                lower,
            )?;
            lower.record_monomorph_call(
                fqn.to_string(),
                &chosen.sig.decl_file,
                chosen.sig.decl_span,
                &type_args,
                &eff_args,
                call_expr.span,
            );
            lower.record_top_level_fun_call_binding(
                call_expr.span,
                ast::TopLevelFunCallBinding {
                    fqn: fqn.to_string(),
                    decl_file: chosen.sig.decl_file.clone(),
                    decl_span: chosen.sig.decl_span,
                    is_intrinsic: chosen.sig.is_intrinsic,
                    intrinsic_entry_name: chosen.sig.intrinsic_entry_name.clone(),
                    type_args,
                    eff_args,
                },
            );
            if let Some(binding) =
                call_arg_binding_from_mapping_with_receiver(&chosen.mapping, &chosen_call_args)
            {
                lower.record_typechecked_call_arg_binding(call_expr.span, binding);
            }
            if chosen.used_unit_sugar {
                lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
            }

            let ret = if safe {
                lower.ty_option(chosen.instantiated.return_ty)
            } else {
                chosen.instantiated.return_ty
            };

            return Ok(ret);
        }
    }

    // T0130：bound 驱动的方法分发——当 receiver 为 TypeKind::Param 时，
    // 通过 where 约束查找 bound 接口的方法集合。
    if let TypeKind::Param(p) = lower.type_kind(actual_receiver_ty) {
        let param_name = p.name.clone();

        if let Some(ret) = try_infer_where_bound_method_call(
            source,
            call_expr,
            receiver,
            actual_receiver_ty,
            &param_name,
            member,
            args,
            explicit_type_args,
            safe,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )? {
            return Ok(ret);
        }
    }

    if !matches!(
        member.resolved.as_ref(),
        Some(ast::ResolvedMemberRef::Fun { .. } | ast::ResolvedMemberRef::ExtensionFun { .. })
    ) {
        let value_resolved = super::super::member::resolve_member_value_target_for_receiver(
            inputs,
            receiver,
            Some(actual_receiver_ty),
            member,
            lower,
        );
        if matches!(
            value_resolved.as_ref(),
            Some(
                ast::ResolvedMemberRef::Value { .. }
                    | ast::ResolvedMemberRef::ExtensionValue { .. }
            )
        ) {
            if let Some(resolved) = value_resolved.as_ref() {
                lower.record_typechecked_member_resolution(member.span, resolved.clone());
            }

            let callee_ty = super::super::member::infer_member_access_ty_from_known_receiver(
                inputs,
                Some(actual_receiver_ty),
                member,
                value_resolved.as_ref(),
                lower,
            )?;
            lower.record_inferred_expr_ty(
                Span::new(receiver.span.start, member.span.end),
                callee_ty,
            );

            if is_funptr_type(callee_ty, lower) {
                return infer_funptr_type_call_expr_type(
                    inputs,
                    call_expr,
                    member_name,
                    callee_ty,
                    args,
                    lower,
                );
            }

            if matches!(
                lower.type_kind(callee_ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            ) {
                return infer_function_type_call_expr_type(
                    inputs,
                    call_expr,
                    member_name,
                    callee_ty,
                    args,
                    lower,
                );
            }

            let callee = match value_resolved.as_ref() {
                Some(ast::ResolvedMemberRef::Value { fqn })
                | Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => fqn.clone(),
                _ => member_name.to_string(),
            };
            return Err(ExprTypeError::CalleeNotCallable {
                callee,
                span: member.span.into(),
            });
        }
    }

    // 当前阶段只支持"扩展函数调用"（T0312）：`receiver.member(args...)`。
    // - 若 resolver 已写回 `ExtensionFun`，优先使用；
    // - 否则（例如 `receiver` 为 `T?` 时 resolver 无法静态确定 receiver 类型），
    //   尝试在"当前包"内按同名顶层 fun 查找 receiver fun。
    let extension_fqns = match if current_lambda_this {
        None
    } else {
        member.resolved.as_ref()
    } {
        Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => member
            .call
            .as_ref()
            .map(|call| {
                call.candidates
                    .iter()
                    .filter_map(|candidate| match candidate {
                        ast::CallCandidate::Fun { fqn } => Some(fqn.clone()),
                        ast::CallCandidate::Constructor { .. } => None,
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|fqns| !fqns.is_empty())
            .unwrap_or_else(|| vec![fqn.clone()]),
        Some(ast::ResolvedMemberRef::Fun { fqn })
        | Some(ast::ResolvedMemberRef::Value { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: fqn.clone(),
                span: member.span.into(),
            });
        }
        None => {
            // resolver 无法静态确定 receiver 类型时（例如 `Shared.t1Go.recv()` 这类非裸 ident receiver），
            // `member.resolved` 可能为空；此时在 typecheck 阶段用"已推导出的 receiver 类型 + import 表"
            // 再做一次 extension fun 查找（与 resolver 的 extension fallback 规则保持一致）。

            // T1317f2：`List/MutableList` 等为 typealias（resolver 侧按名义 FQN 匹配，这里做同样归一化）。
            fn normalize_collections_alias(fqn: &str) -> &str {
                match fqn {
                    "scoop.core.List" => "scoop.core.Array",
                    "scoop.core.MutableList" => "scoop.core.MutableArray",
                    "scoop.collections.Set" => "scoop.core.Array",
                    "scoop.collections.MapView" => "scoop.core.Array",
                    "scoop.collections.MutableSet" => "scoop.core.MutableArray",
                    "scoop.collections.MutableMap" => "scoop.core.MutableArray",
                    _ => fqn,
                }
            }

            let name = source.slice(member.span);
            let use_cone = lower.index().cone_of_source(source);

            let receiver_ty_fqn = match lower.type_kind(actual_receiver_ty) {
                TypeKind::Ref(RefTypeKind::Nominal(n))
                | TypeKind::Value(ValueTypeKind::Nominal(n)) => Some(n.fqn),
                _ => None,
            };
            let receiver_ty_fqn_norm = receiver_ty_fqn.as_deref().map(normalize_collections_alias);

            let mut candidates: Vec<String> = Vec::new();

            let imports = lower.imports();

            // 1) 同包（同 cone）隐式可见。
            for ext in &lower.index().extension_funs {
                if ext.decl_cone != use_cone {
                    continue;
                }
                if ext.pkg_prefix != lower.pkg_prefix() {
                    continue;
                }
                if ext.name != name {
                    continue;
                }

                let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                    Some(ext_receiver) => {
                        ext_receiver == "scoop.core.Any"
                            || receiver_ty_fqn_norm
                                .is_some_and(|r| normalize_collections_alias(ext_receiver) == r)
                    }
                    None => ext.receiver_is_type_param,
                };
                if !receiver_matches {
                    continue;
                }

                let Some(syms) = lower.index().by_fqn.get(&ext.fqn) else {
                    continue;
                };
                if syms
                    .fun
                    .iter()
                    .any(|o| is_symbol_visible_from_source(use_cone, source, &o.symbol))
                {
                    candidates.push(ext.fqn.clone());
                }
            }

            // 2) star import：`import pkg.*`。
            for prefix in &imports.star {
                for ext in &lower.index().extension_funs {
                    if ext.pkg_prefix != *prefix {
                        continue;
                    }
                    if ext.name != name {
                        continue;
                    }

                    let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                        Some(ext_receiver) => {
                            ext_receiver == "scoop.core.Any"
                                || receiver_ty_fqn_norm
                                    .is_some_and(|r| normalize_collections_alias(ext_receiver) == r)
                        }
                        None => ext.receiver_is_type_param,
                    };
                    if !receiver_matches {
                        continue;
                    }

                    let Some(syms) = lower.index().by_fqn.get(&ext.fqn) else {
                        continue;
                    };
                    if syms
                        .fun
                        .iter()
                        .any(|o| is_symbol_visible_from_source(use_cone, source, &o.symbol))
                    {
                        candidates.push(ext.fqn.clone());
                    }
                }
            }

            // 3) 显式 import（含 alias）：通过 local 名字 → fqn 查找 extension。
            if let Some(imported) = imports.value.explicit.get(name) {
                for imported_fqn in imported {
                    for ext in lower
                        .index()
                        .extension_funs
                        .iter()
                        .filter(|e| e.fqn == *imported_fqn)
                    {
                        let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                            Some(ext_receiver) => {
                                ext_receiver == "scoop.core.Any"
                                    || receiver_ty_fqn_norm.is_some_and(|r| {
                                        normalize_collections_alias(ext_receiver) == r
                                    })
                            }
                            None => ext.receiver_is_type_param,
                        };
                        if !receiver_matches {
                            continue;
                        }

                        let Some(syms) = lower.index().by_fqn.get(&ext.fqn) else {
                            continue;
                        };
                        if syms
                            .fun
                            .iter()
                            .any(|o| is_symbol_visible_from_source(use_cone, source, &o.symbol))
                        {
                            candidates.push(ext.fqn.clone());
                        }
                    }
                }
            }

            candidates.sort();
            candidates.dedup();

            match candidates.len() {
                0 => vec![if lower.pkg_prefix().is_empty() {
                    name.to_string()
                } else {
                    format!("{}.{}", lower.pkg_prefix(), name)
                }],
                _ => candidates,
            }
        }
    };
    let extension_candidate_storage = collect_fun_sig_candidates_for_fqns(
        extension_fqns.clone(),
        inputs.source,
        top_level_funs,
        lower,
        builtins,
    )?;
    let ext_candidates: Vec<&CandidateFunSig> = extension_candidate_storage
        .iter()
        .filter(|candidate| candidate.sig.is_extension)
        .collect();
    let Some(first_ext_candidate) = ext_candidates.first().copied() else {
        let callee = extension_fqns
            .first()
            .cloned()
            .unwrap_or_else(|| member_name.to_string());
        return Err(ExprTypeError::CalleeNotCallable {
            callee,
            span: member.span.into(),
        });
    };
    let callee_fqn = first_ext_candidate.fqn.clone();
    let sig = &first_ext_candidate.sig;
    lower.record_typechecked_member_resolution(
        member.span,
        ast::ResolvedMemberRef::ExtensionFun {
            fqn: callee_fqn.clone(),
        },
    );

    // 预先推导所有"显式实参"的类型（不含 receiver），并归一化 named arg 的语法糖节点，
    // 以便在重载筛选中复用这份结果并避免把子表达式错误吞掉。
    let call_args = collect_call_arg_infos(inputs, args, lower)?;
    let synthesized_unit_args = args
        .is_empty()
        .then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
    let sugar_call_args = if let Some(synthesized_args) = synthesized_unit_args.as_ref() {
        Some(collect_call_arg_infos(inputs, synthesized_args, lower)?)
    } else {
        None
    };
    if funptr_invoke_rejects_named_args(&callee_fqn, actual_receiver_ty, lower)
        && let Some(span) = first_named_arg_span(&call_args)
    {
        return Err(ExprTypeError::NamedArgsNotSupportedForCallableType {
            callee: member_name.to_string(),
            span: span.into(),
        });
    }
    check_call_arg_named_rules(&callee_fqn, &call_args)?;
    check_call_named_args_exist_in_any_candidate(
        &callee_fqn,
        &call_args,
        ext_candidates
            .iter()
            .filter_map(|c| c.sig.param_names.get(1..)),
    )?;

    let Some(expected_receiver_ty) = sig.params.first().copied() else {
        // 健壮性：扩展函数至少应该包含 receiver 这一参数。
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    // 只有一个扩展候选：沿用旧的"给出精确 mismatch 诊断"的路径，但补齐命名实参映射（T0453）。
    if ext_candidates.len() == 1 {
        check_unsafe_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
        check_nogc_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
        emit_deprecated_call_warning(&callee_fqn, sig, call_expr.span, lower);
        let expected_args = sig.params.len().saturating_sub(1);

        let Some(param_names) = sig.param_names.get(1..) else {
            // 健壮性：扩展函数至少应该包含 receiver 的占位形参名。
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };
        let Some(param_has_defaults) = sig.param_has_defaults.get(1..) else {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };
        let Some(param_is_vararg) = sig.param_is_vararg.get(1..) else {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };

        let Some((user_param_tys, _, _)) = user_visible_param_slices_after_receiver(
            &sig.params,
            &sig.param_has_defaults,
            &sig.param_is_vararg,
        ) else {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };
        let used_unit_sugar = can_use_zero_arg_unit_call_sugar(
            args,
            user_param_tys,
            param_has_defaults,
            param_is_vararg,
            lower,
        );
        let effective_call_args = if used_unit_sugar {
            sugar_call_args
                .as_ref()
                .expect("typed Unit sugar 选择的扩展调用应有合成实参")
        } else {
            &call_args
        };

        let has_vararg = vararg_param_index(param_is_vararg).is_some();
        if !has_vararg && effective_call_args.len() > expected_args {
            return Err(ExprTypeError::CallArityMismatch {
                callee: callee_fqn,
                expected: expected_args,
                found: effective_call_args.len(),
                span: call_expr.span.into(),
            });
        }

        let required = if has_vararg {
            required_param_count(param_has_defaults, param_is_vararg)
                .unwrap_or_else(|| param_has_defaults.iter().filter(|d| !**d).count())
        } else {
            param_has_defaults.iter().filter(|d| !**d).count()
        };
        if effective_call_args.len() < required {
            return Err(ExprTypeError::CallArityMismatch {
                callee: callee_fqn,
                expected: required,
                found: effective_call_args.len(),
                span: call_expr.span.into(),
            });
        }

        let mapping: Vec<ParamArgBinding> = if !has_vararg {
            let Some(mapping) = map_call_args_to_params_with_defaults(
                effective_call_args,
                param_names,
                param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_fqn,
                    span: call_expr.span.into(),
                });
            };
            mapping
                .into_iter()
                .map(|arg_idx| arg_idx.map_or(ParamArgBinding::Default, ParamArgBinding::Single))
                .collect()
        } else {
            let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                effective_call_args,
                param_names,
                param_has_defaults,
                param_is_vararg,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_fqn,
                    span: call_expr.span.into(),
                });
            };
            mapping
        };

        // spread 实参只能绑定到 vararg 形参。
        for binding in mapping.iter() {
            if let ParamArgBinding::Single(arg_idx) = binding
                && effective_call_args
                    .get(*arg_idx)
                    .is_some_and(|a| a.is_spread)
            {
                return Err(ExprTypeError::SpreadArgRequiresVararg {
                    callee: callee_fqn.clone(),
                    span: effective_call_args[*arg_idx].expr.span.into(),
                });
            }
        }
        let mapping_pairs = expand_param_arg_pairs(&mapping);

        let mut arg_constraints: Vec<GenericArgConstraint> = Vec::new();
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let sig_param_idx = param_idx + 1; // 跳过 receiver
            let arg = &effective_call_args[arg_idx];
            if arg.is_spread {
                if !sig
                    .param_is_vararg
                    .get(sig_param_idx)
                    .copied()
                    .unwrap_or(false)
                {
                    return Err(ExprTypeError::SpreadArgRequiresVararg {
                        callee: callee_fqn.clone(),
                        span: arg.expr.span.into(),
                    });
                }

                let Some(elem_tys) = spread_operand_element_types(arg.ty, lower) else {
                    return Err(ExprTypeError::VarargSpreadRequiresArrayOrTuple {
                        found: lower.fmt_type(arg.ty),
                        hint: vararg_spread_missing_bridge_hint(arg.ty, lower, builtins),
                        span: arg.expr.span.into(),
                    });
                };
                for found_elem in elem_tys {
                    arg_constraints.push(GenericArgConstraint {
                        expected: sig.params[sig_param_idx],
                        found: found_elem,
                        found_is_placeholder: false,
                        from: format!("第 {} 个实参（spread）", arg_idx + 1),
                        span: arg.expr.span,
                    });
                }
                continue;
            }

            arg_constraints.push(GenericArgConstraint {
                expected: sig.params[sig_param_idx],
                found: arg.ty,
                found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                from: format!("第 {} 个实参", arg_idx + 1),
                span: arg.expr.span,
            });
        }

        let mut instantiated = instantiate_fun_sig_for_call_with_optional_explicit_type_args(
            &callee_fqn,
            call_expr.span,
            sig,
            explicit_type_args,
            std::iter::once(GenericArgConstraint {
                expected: expected_receiver_ty,
                found: actual_receiver_ty,
                found_is_placeholder: false,
                from: "接收者（receiver）".to_string(),
                span: receiver.span,
            })
            .chain(arg_constraints),
            lower,
            builtins,
        )?;

        // T0129：检查 where 约束。
        check_fun_where_constraints_after_instantiation(
            &callee_fqn,
            call_expr.span,
            sig,
            &instantiated.type_args,
            lower,
            builtins,
        )?;

        // receiver mismatch 检查：
        // - 默认路径：在推断 `eff` row 参数之前就可以做 receiver 可赋值检查，给出更精确诊断；
        // - 但当 receiver 的期望类型依赖 `E`（例如 `Type<eff (E + IO)>`，或更深的嵌套位置）时，
        //   receiver 的"期望类型"必须等到 `E` 被实例化后才能确定（T0624）。
        let receiver_uses_eff = sig.eff_param.is_some()
            && sig
                .param_eff_row_var_subst
                .first()
                .is_some_and(|p| p.uses_eff_var());
        if !receiver_uses_eff {
            let expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(expected_receiver_ty);
            if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
                return Err(ExprTypeError::CallReceiverTypeMismatch {
                    callee: callee_fqn,
                    expected: lower.fmt_type(expected_receiver_ty),
                    found: lower.fmt_type(actual_receiver_ty),
                    span: receiver.span.into(),
                });
            }
            check_fn_value_to_any_erasure_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
            check_nogc_boxing_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
        }

        // 先在"期望类型语境"下推导每个显式实参的最终类型（lambda 会在此处被真正类型检查）。
        let mut checked_arg_tys: Vec<TypeId> = effective_call_args.iter().map(|a| a.ty).collect();
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &effective_call_args[arg_idx];
            if arg.is_spread {
                continue;
            }
            let found_ty = inputs.infer_in_expected(
                lower,
                arg.expr,
                expected_ty,
                ExpectedTypeFrom::new(format!(
                    "`{}` 的第 {} 个形参 `{}`",
                    callee_fqn,
                    param_idx + 2,
                    sig.param_names[param_idx + 1]
                )),
            )?;
            checked_arg_tys[arg_idx] = found_ty;
        }

        // T0509/T0624/T0628a：推断 `eff` row 参数：
        // - 从 lambda body 的 required effects 推断（`found - base`）；
        // - 从 `Type<eff Row>` receiver/形参的实参类型提取 row 约束（`found - base`）。
        let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.cloned() {
            explicit_eff_arg
        } else if let Some(eff_param) = &sig.eff_param {
            let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

            // receiver 约束：`ReceiverType<eff Row>`。
            if let Some(base) = sig
                .param_nominal_eff_eff_base
                .first()
                .and_then(|b| b.as_ref())
            {
                let base = substitute_type_args_in_effect_row(
                    base.clone(),
                    &sig.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                )?;
                if let Some(found_row) = nominal_eff_row_from_type(actual_receiver_ty, lower) {
                    let delta = effect_row_difference(&found_row, &base);
                    terms.extend(delta.terms);
                }
            }

            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                let arg = &effective_call_args[arg_idx];
                if arg.is_spread {
                    continue;
                }
                let sig_param_idx = param_idx + 1; // 跳过 receiver

                // `Type<eff Row>` 形参约束。
                if let Some(base) = sig
                    .param_nominal_eff_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                {
                    let base = substitute_type_args_in_effect_row(
                        base.clone(),
                        &sig.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    )?;
                    let found_ty = checked_arg_tys[arg_idx];
                    if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                        let delta = effect_row_difference(&found_row, &base);
                        terms.extend(delta.terms);
                    }
                }

                let Some(base) = sig
                    .param_fn_effect_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                else {
                    continue;
                };
                if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                    continue;
                }

                let base = substitute_type_args_in_effect_row(
                    base.clone(),
                    &sig.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                )?;
                let found_ty = checked_arg_tys[arg_idx];
                if let TypeKind::Ref(RefTypeKind::Function(found_fun)) = lower.type_kind(found_ty) {
                    let delta = effect_row_difference(&found_fun.effects, &base);
                    terms.extend(delta.terms);
                }
            }

            let inferred = EffectRow::new(terms);
            substitute_type_args_in_effect_row(
                inferred,
                &sig.type_params,
                &instantiated.type_args,
                lower,
                call_expr.span,
            )?
        } else {
            EffectRow::pure()
        };

        instantiate_eff_row_var_in_sig_types(
            sig,
            &mut instantiated,
            &eff_arg,
            lower,
            call_expr.span,
        )?;

        // 若 receiver 依赖 `E`，现在 `E` 已实例化完毕，补做 receiver mismatch 检查。
        if receiver_uses_eff {
            let expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(expected_receiver_ty);
            if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
                return Err(ExprTypeError::CallReceiverTypeMismatch {
                    callee: callee_fqn,
                    expected: lower.fmt_type(expected_receiver_ty),
                    found: lower.fmt_type(actual_receiver_ty),
                    span: receiver.span.into(),
                });
            }
            check_fn_value_to_any_erasure_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
            check_nogc_boxing_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
        }

        // 再做"可赋值"检查（此时 lambda 的 effects 也已经被推断并写入 found_ty）。
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &effective_call_args[arg_idx];
            let found_ty = checked_arg_tys[arg_idx];

            if arg.is_spread {
                let sig_param_idx = param_idx + 1;
                if !sig
                    .param_is_vararg
                    .get(sig_param_idx)
                    .copied()
                    .unwrap_or(false)
                {
                    return Err(ExprTypeError::SpreadArgRequiresVararg {
                        callee: callee_fqn.clone(),
                        span: arg.expr.span.into(),
                    });
                }

                let Some(elem_tys) = spread_operand_element_types(found_ty, lower) else {
                    return Err(ExprTypeError::VarargSpreadRequiresArrayOrTuple {
                        found: lower.fmt_type(found_ty),
                        hint: vararg_spread_missing_bridge_hint(found_ty, lower, builtins),
                        span: arg.expr.span.into(),
                    });
                };
                for elem_ty in elem_tys {
                    if is_type_assignable(elem_ty, expected_ty, lower, builtins) {
                        check_fn_value_to_any_erasure_gate(
                            elem_ty,
                            expected_ty,
                            arg.expr.span,
                            lower,
                            builtins,
                        )?;
                        continue;
                    }
                    return Err(ExprTypeError::VarargSpreadElementTypeMismatch {
                        expected: lower.fmt_type(expected_ty),
                        found: lower.fmt_type(elem_ty),
                        span: arg.expr.span.into(),
                    });
                }
                continue;
            }

            if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                check_fn_value_to_any_erasure_gate(
                    found_ty,
                    expected_ty,
                    arg.expr.span,
                    lower,
                    builtins,
                )?;
                check_nogc_boxing_gate(found_ty, expected_ty, arg.expr.span, lower, builtins)?;
                continue;
            }
            if literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins) {
                continue;
            }

            return Err(ExprTypeError::CallArgTypeMismatch {
                callee: callee_fqn,
                // extension 调用：`receiver.member(arg1, arg2, ...)` 的第 1 个"显式参数"
                // 对应 `sig.params[1]`（跳过 receiver 参数）。
                index: param_idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.expr.span.into(),
            });
        }

        // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
        let type_param_bindings = type_param_bindings_from_sig(&sig.type_params, lower);
        let eff_bindings: Vec<(String, EffectRow)> = sig
            .eff_param
            .as_ref()
            .map(|p| vec![(p.name.clone(), eff_arg.clone())])
            .unwrap_or_default();
        let lowered_effects = lower.lower_effect_row_expr_in_decl_file_with_scopes(
            &sig.decl_file,
            type_param_bindings,
            eff_bindings,
            sig.effects.as_ref(),
        );
        let call_effects = substitute_type_args_in_effect_row(
            lowered_effects?,
            &sig.type_params,
            &instantiated.type_args,
            lower,
            call_expr.span,
        )?;
        for effect in call_effects.terms.iter().copied() {
            lower.record_performed_effect(effect, call_expr.span);
        }

        // T0712/T5000e2b：记录带 receiver 的 direct-call 实例请求。
        // 对 generic owner member/getter，这里需要把 owner-specialization 的 concrete args
        // 放在函数自身 type args 之前，形成可复用的实例身份。
        let eff_args = sig
            .eff_param
            .as_ref()
            .map(|_| vec![eff_arg.clone()])
            .unwrap_or_default();
        let type_args = combined_member_instance_type_args(
            &callee_fqn,
            actual_receiver_ty,
            &instantiated.type_args,
            lower,
        )?;
        lower.record_monomorph_call(
            callee_fqn.clone(),
            &sig.decl_file,
            sig.decl_span,
            &type_args,
            &eff_args,
            call_expr.span,
        );
        lower.record_top_level_fun_call_binding(
            call_expr.span,
            ast::TopLevelFunCallBinding {
                fqn: callee_fqn.clone(),
                decl_file: sig.decl_file.clone(),
                decl_span: sig.decl_span,
                is_intrinsic: sig.is_intrinsic,
                intrinsic_entry_name: sig.intrinsic_entry_name.clone(),
                type_args,
                eff_args,
            },
        );
        if let Some(binding) =
            call_arg_binding_from_mapping_with_receiver_prefix(&mapping, effective_call_args)
        {
            lower.record_typechecked_call_arg_binding(call_expr.span, binding);
        }
        if used_unit_sugar {
            lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
        }

        let ret = if safe {
            lower.ty_option(instantiated.return_ty)
        } else {
            instantiated.return_ty
        };

        return Ok(ret);
    }

    #[derive(Debug, Clone)]
    struct MatchedExtensionOverload<'a> {
        fqn: &'a str,
        sig: &'a FunSigOwned,
        instantiated: InstantiatedFunSig,
        eff_arg: EffectRow,
        receiver_ty: TypeId,
        /// `call_args[arg_idx]` 对应的"期望类型"（排除了 receiver 参数）。
        expected_arg_tys: Vec<TypeId>,
        /// 调用点需要用默认值补齐的形参个数（越少越"具体"）。
        defaults_used: usize,
        /// 形参 -> 实参绑定（不含 receiver，receiver 由调用形状隐式提供）。
        mapping: Vec<ParamArgBinding>,
        /// 当前候选是否通过 typed `Unit` zero-arg sugar 匹配得到。
        used_unit_sugar: bool,
    }

    fn is_strictly_more_specific_extension_overload(
        a: &MatchedExtensionOverload<'_>,
        b: &MatchedExtensionOverload<'_>,
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> bool {
        let a_le_b = is_type_assignable(a.receiver_ty, b.receiver_ty, lower, builtins)
            && a.expected_arg_tys
                .iter()
                .zip(b.expected_arg_tys.iter())
                .all(|(a_ty, b_ty)| is_type_assignable(*a_ty, *b_ty, lower, builtins));
        let b_le_a = is_type_assignable(b.receiver_ty, a.receiver_ty, lower, builtins)
            && b.expected_arg_tys
                .iter()
                .zip(a.expected_arg_tys.iter())
                .all(|(b_ty, a_ty)| is_type_assignable(*b_ty, *a_ty, lower, builtins));

        a_le_b && !b_le_a
    }

    fn pick_most_specific_extension_overload(
        candidates: &[MatchedExtensionOverload<'_>],
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Option<usize> {
        for (idx, cand) in candidates.iter().enumerate() {
            let mut ok = true;
            for (other_idx, other) in candidates.iter().enumerate() {
                if idx == other_idx {
                    continue;
                }
                if !is_strictly_more_specific_extension_overload(cand, other, lower, builtins) {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(idx);
            }
        }

        // tie-break：默认参数更少者优先（"非默认参数优先"）。
        let min_defaults = candidates
            .iter()
            .map(|c| c.defaults_used)
            .min()
            .unwrap_or(0);
        let mut it = candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.defaults_used == min_defaults);
        let (idx, _) = it.next()?;
        if it.next().is_some() {
            return None;
        }
        Some(idx)
    }

    // 多候选：先按 receiver/参数匹配筛选，再用 receiver/参数 specificity 选出 most-specific（T0455）。
    let mut matched: Vec<MatchedExtensionOverload<'_>> = Vec::new();

    for candidate in ext_candidates.iter().copied() {
        let callee_fqn = candidate.fqn.as_str();
        let cand = &candidate.sig;
        let Some((user_param_tys, param_has_defaults, param_is_vararg)) =
            user_visible_param_slices_after_receiver(
                &cand.params,
                &cand.param_has_defaults,
                &cand.param_is_vararg,
            )
        else {
            continue;
        };
        let Some(param_names) = cand.param_names.get(1..) else {
            continue;
        };

        let exact_mapping = map_call_args_to_params_with_defaults_and_varargs(
            &call_args,
            param_names,
            param_has_defaults,
            param_is_vararg,
        );
        let (call_args_for_candidate, mapping, used_unit_sugar) =
            if let Some(mapping) = exact_mapping {
                (&call_args, mapping, false)
            } else if can_use_zero_arg_unit_call_sugar(
                args,
                user_param_tys,
                param_has_defaults,
                param_is_vararg,
                lower,
            ) {
                let Some(sugar_call_args) = sugar_call_args.as_ref() else {
                    continue;
                };
                let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                    sugar_call_args,
                    param_names,
                    param_has_defaults,
                    param_is_vararg,
                ) else {
                    continue;
                };
                (sugar_call_args, mapping, true)
            } else {
                continue;
            };

        // spread 实参只能绑定到 vararg 形参；否则该候选不匹配。
        let mut ok = true;
        for binding in mapping.iter() {
            match binding {
                ParamArgBinding::Default => {}
                ParamArgBinding::Single(arg_idx) => {
                    if call_args_for_candidate
                        .get(*arg_idx)
                        .is_some_and(|a| a.is_spread)
                    {
                        ok = false;
                        break;
                    }
                }
                ParamArgBinding::Vararg(_) => {}
            }
        }
        if !ok {
            continue;
        }

        let mapping_pairs = expand_param_arg_pairs(&mapping);

        let mut arg_constraints: Vec<GenericArgConstraint> = Vec::new();
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let sig_param_idx = param_idx + 1; // 跳过 receiver
            let arg = &call_args_for_candidate[arg_idx];
            if arg.is_spread {
                if !cand
                    .param_is_vararg
                    .get(sig_param_idx)
                    .copied()
                    .unwrap_or(false)
                {
                    ok = false;
                    break;
                }
                let Some(elem_tys) = spread_operand_element_types(arg.ty, lower) else {
                    ok = false;
                    break;
                };
                for found_elem in elem_tys {
                    arg_constraints.push(GenericArgConstraint {
                        expected: cand.params[sig_param_idx],
                        found: found_elem,
                        found_is_placeholder: false,
                        from: format!("第 {} 个实参（spread）", arg_idx + 1),
                        span: arg.expr.span,
                    });
                }
                continue;
            }

            arg_constraints.push(GenericArgConstraint {
                expected: cand.params[sig_param_idx],
                found: arg.ty,
                found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                from: format!("第 {} 个实参", arg_idx + 1),
                span: arg.expr.span,
            });
        }
        if !ok {
            continue;
        }

        let mut instantiated = match instantiate_fun_sig_for_call_with_optional_explicit_type_args(
            callee_fqn,
            call_expr.span,
            cand,
            explicit_type_args,
            std::iter::once(GenericArgConstraint {
                expected: cand.params[0],
                found: actual_receiver_ty,
                found_is_placeholder: false,
                from: "接收者（receiver）".to_string(),
                span: receiver.span,
            })
            .chain(arg_constraints),
            lower,
            builtins,
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // T0129：检查 where 约束；不满足则跳过该候选。
        if check_fun_where_constraints_after_instantiation(
            callee_fqn,
            call_expr.span,
            cand,
            &instantiated.type_args,
            lower,
            builtins,
        )
        .is_err()
        {
            continue;
        }

        // receiver mismatch 检查：同单候选路径，若 receiver 的期望类型依赖 `E`，
        // 必须等到 `E` 推断/实例化后才能确定 receiver 是否匹配（T0624）。
        let receiver_uses_eff = cand.eff_param.is_some()
            && cand
                .param_eff_row_var_subst
                .first()
                .is_some_and(|p| p.uses_eff_var());
        let mut cand_expected_receiver_ty = instantiated
            .params
            .first()
            .copied()
            .unwrap_or(cand.params[0]);
        if !receiver_uses_eff
            && !is_type_assignable(
                actual_receiver_ty,
                cand_expected_receiver_ty,
                lower,
                builtins,
            )
        {
            continue;
        }

        // 只在需要时（lambda）进入 expected-context typecheck（与 direct call 多候选路径保持一致）。
        let mut ok = true;
        let mut checked_arg_tys: Vec<TypeId> =
            call_args_for_candidate.iter().map(|a| a.ty).collect();
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let arg = &call_args_for_candidate[arg_idx];
            if arg.is_spread {
                continue;
            }
            if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                continue;
            }

            let expected_ty = instantiated.params[param_idx + 1];
            let found_ty = match inputs.infer_in_expected(
                lower,
                arg.expr,
                expected_ty,
                ExpectedTypeFrom::new(format!(
                    "`{}` 的第 {} 个形参 `{}`",
                    callee_fqn,
                    param_idx + 2,
                    cand.param_names[param_idx + 1]
                )),
            ) {
                Ok(ty) => ty,
                Err(_) => {
                    ok = false;
                    break;
                }
            };
            checked_arg_tys[arg_idx] = found_ty;
        }
        if !ok {
            continue;
        }

        // T0509/T0624/T0628a：推断 `eff` row 参数：
        // - 从 lambda body 的 required effects 推断（`found - base`）；
        // - 从 `Type<eff Row>` receiver/形参的实参类型提取 row 约束（`found - base`）。
        let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.cloned() {
            explicit_eff_arg
        } else if let Some(eff_param) = &cand.eff_param {
            let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

            if let Some(base) = cand
                .param_nominal_eff_eff_base
                .first()
                .and_then(|b| b.as_ref())
            {
                let base = match substitute_type_args_in_effect_row(
                    base.clone(),
                    &cand.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                ) {
                    Ok(row) => row,
                    Err(_) => continue,
                };
                if let Some(found_row) = nominal_eff_row_from_type(actual_receiver_ty, lower) {
                    let delta = effect_row_difference(&found_row, &base);
                    terms.extend(delta.terms);
                }
            }

            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                let arg = &call_args_for_candidate[arg_idx];
                if arg.is_spread {
                    continue;
                }
                let sig_param_idx = param_idx + 1; // 跳过 receiver

                if let Some(base) = cand
                    .param_nominal_eff_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                {
                    let base = match substitute_type_args_in_effect_row(
                        base.clone(),
                        &cand.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    ) {
                        Ok(row) => row,
                        Err(_) => continue,
                    };
                    let found_ty = checked_arg_tys[arg_idx];
                    if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                        let delta = effect_row_difference(&found_row, &base);
                        terms.extend(delta.terms);
                    }
                }

                let Some(base) = cand
                    .param_fn_effect_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                else {
                    continue;
                };
                if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                    continue;
                }

                let base = match substitute_type_args_in_effect_row(
                    base.clone(),
                    &cand.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                ) {
                    Ok(row) => row,
                    Err(_) => continue,
                };
                let found_ty = checked_arg_tys[arg_idx];
                if let TypeKind::Ref(RefTypeKind::Function(found_fun)) = lower.type_kind(found_ty) {
                    let delta = effect_row_difference(&found_fun.effects, &base);
                    terms.extend(delta.terms);
                }
            }

            let inferred = EffectRow::new(terms);
            match substitute_type_args_in_effect_row(
                inferred,
                &cand.type_params,
                &instantiated.type_args,
                lower,
                call_expr.span,
            ) {
                Ok(row) => row,
                Err(_) => continue,
            }
        } else {
            EffectRow::pure()
        };

        if cand.eff_param.is_some()
            && instantiate_eff_row_var_in_sig_types(
                cand,
                &mut instantiated,
                &eff_arg,
                lower,
                call_expr.span,
            )
            .is_err()
        {
            ok = false;
        }
        if !ok {
            continue;
        }

        // 若 receiver 依赖 `E`，现在 `E` 已实例化完毕，补做 receiver mismatch 检查。
        if receiver_uses_eff {
            cand_expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(cand.params[0]);
            if !is_type_assignable(
                actual_receiver_ty,
                cand_expected_receiver_ty,
                lower,
                builtins,
            ) {
                continue;
            }
        }

        // 参数可赋值检查（跳过 receiver；只检查显式实参）。
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &call_args_for_candidate[arg_idx];
            let found_ty = checked_arg_tys[arg_idx];

            if arg.is_spread {
                let sig_param_idx = param_idx + 1;
                if !cand
                    .param_is_vararg
                    .get(sig_param_idx)
                    .copied()
                    .unwrap_or(false)
                {
                    ok = false;
                    break;
                }
                let Some(elem_tys) = spread_operand_element_types(found_ty, lower) else {
                    ok = false;
                    break;
                };
                for elem_ty in elem_tys {
                    if is_type_assignable(elem_ty, expected_ty, lower, builtins) {
                        continue;
                    }
                    ok = false;
                    break;
                }
                if !ok {
                    break;
                }
                continue;
            }

            if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                continue;
            }
            if literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins) {
                continue;
            }
            ok = false;
            break;
        }

        if ok {
            let defaults_used = mapping
                .iter()
                .filter(|b| matches!(b, ParamArgBinding::Default))
                .count();
            let mut expected_arg_tys = vec![builtins.nothing; call_args_for_candidate.len()];
            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                expected_arg_tys[arg_idx] = instantiated.params[param_idx + 1];
            }

            matched.push(MatchedExtensionOverload {
                fqn: callee_fqn,
                sig: cand,
                receiver_ty: cand_expected_receiver_ty,
                expected_arg_tys,
                instantiated,
                eff_arg,
                defaults_used,
                mapping,
                used_unit_sugar,
            });
        }
    }

    if matched.iter().any(|cand| !cand.used_unit_sugar) {
        matched.retain(|cand| !cand.used_unit_sugar);
    }

    let chosen = match matched.len() {
        0 => {
            let candidates = join_overload_rejections(
                ext_candidates
                    .iter()
                    .filter_map(|candidate| {
                        let name = short_name_from_fqn(&candidate.fqn).to_string();
                        let cand = &candidate.sig;
                        let param_names = cand.param_names.get(1..)?;
                        let param_has_defaults = cand.param_has_defaults.get(1..)?;
                        let param_is_vararg = cand.param_is_vararg.get(1..)?;
                        let param_tys = cand.params.get(1..)?;
                        Some(OverloadRejection {
                            signature: fmt_overload_signature(
                                &name,
                                cand.params.first().copied(),
                                param_tys,
                                lower,
                            ),
                            location: format_candidate_location(
                                lower,
                                &cand.decl_file,
                                cand.decl_span,
                            ),
                            reason: describe_basic_applicability_rejection(
                                BasicApplicabilityRejection {
                                    call_args: &call_args,
                                    param_names,
                                    param_has_defaults,
                                    param_is_vararg,
                                    param_tys,
                                    source,
                                    lower,
                                    builtins,
                                },
                            ),
                        })
                    })
                    .collect(),
            );
            return Err(ExprTypeError::NoApplicableOverload {
                callee: member_name.to_string(),
                candidates,
                span: call_expr.span.into(),
            });
        }
        1 => matched.pop().expect("len == 1"),
        _ => {
            let Some(idx) = pick_most_specific_extension_overload(&matched, lower, builtins) else {
                let candidates = join_overload_signatures(
                    matched
                        .iter()
                        .map(|c| {
                            let name = short_name_from_fqn(c.fqn).to_string();
                            fmt_overload_signature(
                                &name,
                                Some(c.receiver_ty),
                                c.instantiated.params.get(1..).unwrap_or_default(),
                                lower,
                            )
                        })
                        .collect(),
                );
                return Err(ExprTypeError::AmbiguousOverload {
                    callee: member_name.to_string(),
                    candidates,
                    span: call_expr.span.into(),
                });
            };
            matched.swap_remove(idx)
        }
    };

    let chosen_fqn = chosen.fqn;
    lower.record_typechecked_member_resolution(
        member.span,
        ast::ResolvedMemberRef::ExtensionFun {
            fqn: chosen_fqn.to_string(),
        },
    );
    check_unsafe_call_gate(chosen_fqn, chosen.sig, call_expr.span, lower)?;
    check_nogc_call_gate(chosen_fqn, chosen.sig, call_expr.span, lower)?;
    emit_deprecated_call_warning(chosen_fqn, chosen.sig, call_expr.span, lower);

    // `@NoGC`：已知分配点（boxing）门禁（receiver + 显式实参）。
    check_fn_value_to_any_erasure_gate(
        actual_receiver_ty,
        chosen.receiver_ty,
        receiver.span,
        lower,
        builtins,
    )?;
    check_nogc_boxing_gate(
        actual_receiver_ty,
        chosen.receiver_ty,
        receiver.span,
        lower,
        builtins,
    )?;
    let chosen_call_args = if chosen.used_unit_sugar {
        sugar_call_args
            .as_ref()
            .expect("typed Unit sugar 选择的扩展调用应有合成实参")
    } else {
        &call_args
    };
    for (arg_idx, arg) in chosen_call_args.iter().enumerate() {
        let expected_ty = *chosen
            .expected_arg_tys
            .get(arg_idx)
            .unwrap_or(&builtins.nothing);
        if expected_ty == builtins.nothing {
            continue;
        }
        if is_type_assignable(arg.ty, expected_ty, lower, builtins) {
            check_fn_value_to_any_erasure_gate(
                arg.ty,
                expected_ty,
                arg.expr.span,
                lower,
                builtins,
            )?;
            check_nogc_boxing_gate(arg.ty, expected_ty, arg.expr.span, lower, builtins)?;
        }
    }

    // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
    let type_param_bindings = type_param_bindings_from_sig(&chosen.sig.type_params, lower);
    let eff_bindings: Vec<(String, EffectRow)> = chosen
        .sig
        .eff_param
        .as_ref()
        .map(|p| vec![(p.name.clone(), chosen.eff_arg.clone())])
        .unwrap_or_default();
    let lowered_effects = lower.lower_effect_row_expr_in_decl_file_with_scopes(
        &chosen.sig.decl_file,
        type_param_bindings,
        eff_bindings,
        chosen.sig.effects.as_ref(),
    );
    let call_effects = substitute_type_args_in_effect_row(
        lowered_effects?,
        &chosen.sig.type_params,
        &chosen.instantiated.type_args,
        lower,
        call_expr.span,
    )?;
    for effect in call_effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_expr.span);
    }

    // T0712/T5000e2b：记录带 receiver 的 direct-call 实例请求。
    // 对 generic owner member/getter，这里需要把 owner-specialization 的 concrete args
    // 放在函数自身 type args 之前，形成可复用的实例身份。
    let eff_args = chosen
        .sig
        .eff_param
        .as_ref()
        .map(|_| vec![chosen.eff_arg.clone()])
        .unwrap_or_default();
    let type_args = combined_member_instance_type_args(
        chosen_fqn,
        actual_receiver_ty,
        &chosen.instantiated.type_args,
        lower,
    )?;
    lower.record_monomorph_call(
        chosen_fqn.to_string(),
        &chosen.sig.decl_file,
        chosen.sig.decl_span,
        &type_args,
        &eff_args,
        call_expr.span,
    );
    lower.record_top_level_fun_call_binding(
        call_expr.span,
        ast::TopLevelFunCallBinding {
            fqn: chosen_fqn.to_string(),
            decl_file: chosen.sig.decl_file.clone(),
            decl_span: chosen.sig.decl_span,
            is_intrinsic: chosen.sig.is_intrinsic,
            intrinsic_entry_name: chosen.sig.intrinsic_entry_name.clone(),
            type_args,
            eff_args,
        },
    );
    if let Some(binding) =
        call_arg_binding_from_mapping_with_receiver_prefix(&chosen.mapping, chosen_call_args)
    {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }
    if chosen.used_unit_sugar {
        lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
    }

    let ret = if safe {
        lower.ty_option(chosen.instantiated.return_ty)
    } else {
        chosen.instantiated.return_ty
    };

    Ok(ret)
}
