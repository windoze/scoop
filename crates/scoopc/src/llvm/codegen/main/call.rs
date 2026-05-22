//! Call lowering: codegen_call*, virtual / itable / funptr / native callable, bound args, ordinary param binding, libc bridges.

#![allow(dead_code)]

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn codegen_call(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        // T0125：call expression 的结果 TypeId（用于泛型 class ctor 的 mangled FQN 查找）。
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_call_impl(span, callee, args, expected, result_ty)
    }

    // 原子 intrinsics 需要“真实可寻址的槽位地址”，不能先把 member access 降成 rvalue load。
    pub(in crate::llvm::codegen) fn codegen_addressable_place(
        &mut self,
        expr: &hir::Expr,
    ) -> Result<AddressablePlace<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                let local = self.function_cx.env.get(*id).unwrap_or_else(|| {
                    self.panic_verified_intrinsic_contract(
                        "codegen_addressable_place",
                        "local target missing from function environment",
                    )
                });

                let ptr = self.local_ptr_for_use(expr.span, local, "atomic_int_slot")?;
                Ok(AddressablePlace {
                    ptr,
                    ty: local.ty,
                    writable: local.mutable,
                })
            }
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                if self.lir_global_root_has_kind(fqn, LirGlobalRootKind::ExternGlobal) {
                    let root = self
                        .expect_lir_global_root_kind(
                            fqn,
                            LirGlobalRootKind::ExternGlobal,
                            "codegen_addressable_place",
                        )
                        .clone();
                    let extern_global = root.extern_global.as_ref().unwrap_or_else(|| {
                        panic!("codegen_addressable_place: extern LIR root is missing contract")
                    });
                    let gv = self.declare_lir_extern_global(&root)?;
                    let cg_ty = self.expect_cg_ty_of(
                        self.lir_global_root_ty(&root, "addressable extern global"),
                        "addressable extern global",
                    );
                    return Ok(AddressablePlace {
                        ptr: gv.as_pointer_value(),
                        ty: cg_ty,
                        writable: extern_global.mutable,
                    });
                }

                if !self.lir_global_root_has_kind(fqn, LirGlobalRootKind::TopLevelMutableVar) {
                    self.panic_verified_intrinsic_contract(
                        "codegen_addressable_place",
                        "top-level target metadata missing",
                    );
                }
                let root = self
                    .expect_lir_global_root_kind(
                        fqn,
                        LirGlobalRootKind::TopLevelMutableVar,
                        "codegen_addressable_place",
                    )
                    .clone();

                let gv = self.declare_lir_top_level_var_global(&root)?;
                let cg_ty = self.expect_cg_ty_of(
                    self.lir_global_root_ty(&root, "addressable top-level var"),
                    "addressable top-level var",
                );
                Ok(AddressablePlace {
                    ptr: gv.as_pointer_value(),
                    ty: cg_ty,
                    writable: true,
                })
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
                    self.panic_verified_intrinsic_contract(
                        "codegen_addressable_place",
                        "member target is not a value",
                    );
                };

                let receiver_hir_ty = self
                    .resolve_expr_concrete_type(receiver)
                    .unwrap_or(receiver.ty);
                if let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span, Some(receiver_hir_ty))?
                {
                    let field = class.fields.get(field_idx as usize).unwrap_or_else(|| {
                        panic!(
                            "codegen_addressable_place: verifier accepted class field index drift"
                        )
                    });
                    let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
                    let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
                    let raw = self.expect_cg_value(recv, "addressable class field receiver");
                    let obj_ptr =
                        self.expect_pointer_value(raw, "addressable class field receiver");

                    let ptr =
                        self.codegen_class_field_ptr(member.span, &class, obj_ptr, field_idx)?;
                    return Ok(AddressablePlace {
                        ptr,
                        ty: field_cg,
                        writable: field.mutable,
                    });
                }

                let base = self.codegen_addressable_place(receiver)?;
                let CgTy::Struct(struct_ty) = base.ty else {
                    self.panic_verified_intrinsic_contract(
                        "codegen_addressable_place",
                        "member receiver is not addressable struct storage",
                    );
                };

                let (field_idx, field_ty) =
                    self.lookup_struct_field(struct_ty, fqn, member.span)?;
                let llvm_struct_ty = self.llvm_struct_type(member.span, struct_ty)?;
                let ptr = self.builder.build_struct_gep(
                    llvm_struct_ty,
                    base.ptr,
                    field_idx,
                    "atomic_int_field_gep",
                )?;
                Ok(AddressablePlace {
                    ptr,
                    ty: field_ty,
                    writable: base.writable,
                })
            }
            _ => self.panic_verified_intrinsic_contract(
                "codegen_addressable_place",
                "target is not an addressable lvalue",
            ),
        }
    }

    /// T0127: 从 HIR 表达式中尽量提取具体（非 Any/Param）的 TypeId。
    ///
    /// 对于字面量表达式直接返回其 HIR type；对于变量引用尝试从 env 中获取 hir_ty。
    /// T0130: 对于 Call 表达式，尝试通过 callee 的已知签名推导返回类型。
    pub(in crate::llvm::codegen) fn resolve_expr_concrete_type(
        &self,
        expr: &hir::Expr,
    ) -> Option<crate::ty::TypeId> {
        ExprFactResolver::new(self.types, self.hir_facts.as_ref(), |id| {
            self.function_cx.env.get(id).and_then(|local| local.hir_ty)
        })
        .resolve_expr_concrete_type(expr)
    }

    pub(in crate::llvm::codegen) fn maybe_record_active_suspend_site_effect_outcome(
        &mut self,
        call_span: crate::span::Span,
        outcome_slot: PointerValue<'ctx>,
    ) {
        if let Some(capture) = self.effect_cx.suspend_site_effect_outcomes.active_capture
            && (capture.capture_any || capture.call_span == call_span)
        {
            self.effect_cx
                .suspend_site_effect_outcomes
                .explicit_outcomes
                .insert(capture.site_id, outcome_slot);
        }
    }

    pub(in crate::llvm::codegen) fn codegen_top_level_fun_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_top_level_fun_call_impl(span, callee_span, fqn, args)
    }

    /// 为 native callable 调用点生成 `scoop_enter_native(root_slots, len)`。
    ///
    /// 设计取舍（v0）：
    /// - 这里采用保守策略：把当前 scope 中所有 `Ref/String` locals 的栈槽地址作为 roots slots；
    /// - 这样可以保证 GC 在 native 期间能扫描/更新这些 slots（moving GC 也可写回更新后的指针）；
    /// - 代价是可能多保活一些对象（但不会漏 roots）。
    pub(in crate::llvm::codegen) fn emit_enter_native_for_extern_call(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        self.emit_enter_native_for_extern_call_impl(at)
    }

    pub(in crate::llvm::codegen) fn try_codegen_class_vtable_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        self.try_codegen_class_vtable_call_impl(span, callee_span, fqn, args)
    }

    pub(in crate::llvm::codegen) fn try_codegen_interface_itable_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        self.try_codegen_interface_itable_call_impl(span, callee_span, fqn, args)
    }

    pub(in crate::llvm::codegen) fn load_class_vtable_slot_fn_ptr_i8(
        &mut self,
        _at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.load_class_vtable_slot_fn_ptr_i8_impl(_at, receiver, slot)
    }

    pub(in crate::llvm::codegen) fn llvm_scoop_itable_entry_type(&self) -> StructType<'ctx> {
        self.llvm_scoop_itable_entry_type_impl()
    }

    pub(in crate::llvm::codegen) fn llvm_scoop_itable_type(&self) -> StructType<'ctx> {
        self.llvm_scoop_itable_type_impl()
    }

    pub(in crate::llvm::codegen) fn lookup_interface_itable_slot(
        &mut self,
        at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        interface_id: u64,
        slot: u32,
    ) -> Result<InterfaceItableSlotLookup<'ctx>, LlvmEmitError> {
        self.lookup_interface_itable_slot_impl(at, receiver, interface_id, slot)
    }

    pub(in crate::llvm::codegen) fn load_interface_itable_slot_fn_ptr_i8(
        &mut self,
        at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        interface_id: u64,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        self.load_interface_itable_slot_fn_ptr_i8_impl(at, receiver, interface_id, slot)
    }

    pub(in crate::llvm::codegen) fn codegen_funptr_value_call(
        &mut self,
        funptr_addr: inkwell::values::IntValue<'ctx>,
        funptr_int_ty: IntTy,
        call: FunPtrCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_funptr_value_call_impl(funptr_addr, funptr_int_ty, call)
    }

    /// 把调用点上的 positional/named HIR 实参映射为 `arg_idx -> param_idx`。
    ///
    /// 约束与前端 typecheck 保持一致：
    /// - 一旦出现命名实参，后续不能再出现位置实参；
    /// - 所有命名都必须命中形参；
    /// - 每个形参必须且只能被一个显式实参提供。
    pub(in crate::llvm::codegen) fn map_call_args_to_params_by_name(
        &self,
        param_names: &[String],
        args: &[hir::CallArg],
    ) -> Option<Vec<usize>> {
        self.map_call_args_to_params_by_name_impl(param_names, args)
    }

    /// 在保持源码求值顺序的前提下，把调用点实参求值并归位为"按形参顺序排列"的 LLVM 实参。
    pub(in crate::llvm::codegen) fn codegen_bound_call_args(
        &mut self,
        spec: BoundCallArgsSpec,
        param_names: &[String],
        param_tys: &[TypeId],
        args: &[hir::CallArg],
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        self.codegen_bound_call_args_impl(spec, param_names, param_tys, args)
    }

    pub(in crate::llvm::codegen) fn callable_value_param_names(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> Vec<String> {
        self.callable_value_param_names_impl(fun_ty)
    }

    pub(in crate::llvm::codegen) fn callable_value_param_tys(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> Vec<TypeId> {
        self.callable_value_param_tys_impl(fun_ty)
    }

    pub(in crate::llvm::codegen) fn codegen_callable_value_args(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fun_ty: &crate::ty::FunctionType,
        args: &[hir::CallArg],
        kind: &'static str,
        abi_mode: CallArgAbiMode,
    ) -> Result<Vec<EvaluatedCallArg<'ctx>>, LlvmEmitError> {
        self.codegen_callable_value_args_impl(span, callee_span, fun_ty, args, kind, abi_mode)
    }

    pub(in crate::llvm::codegen) fn codegen_function_value_call(
        &mut self,
        local: &CgLocal<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_function_value_call_impl(local, call)
    }

    pub(in crate::llvm::codegen) fn codegen_function_value_call_from_closure_obj(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
        call: CallableValueCallSpec<'_>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_function_value_call_from_closure_obj_impl(closure_obj_i8, call)
    }

    // 控制流 codegen（if/when 等）已拆分到子模块（T0102d）。

    pub(in crate::llvm::codegen) fn llvm_param_ty(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        self.llvm_param_ty_impl(span, ty)
    }

    pub(in crate::llvm::codegen) fn ordinary_param_abi(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<OrdinaryParamAbi<'ctx>, LlvmEmitError> {
        self.ordinary_param_abi_impl(span, ty)
    }

    pub(in crate::llvm::codegen) fn classify_direct_extern_native_callable(
        &mut self,
        span: crate::span::Span,
        callable_fqn: &str,
        param_tys: &[TypeId],
        return_ty: TypeId,
    ) -> Result<NativeCallableAbi<'ctx>, LlvmEmitError> {
        self.classify_direct_extern_native_callable_impl(span, callable_fqn, param_tys, return_ty)
    }

    pub(in crate::llvm::codegen) fn classify_funptr_native_callable(
        &mut self,
        span: crate::span::Span,
        param_tys: &[TypeId],
        return_ty: TypeId,
    ) -> Result<NativeCallableAbi<'ctx>, LlvmEmitError> {
        self.classify_funptr_native_callable_impl(span, param_tys, return_ty)
    }

    pub(in crate::llvm::codegen) fn emit_native_callable_call(
        &mut self,
        at: crate::span::Span,
        abi: &NativeCallableAbi<'ctx>,
        target: NativeCallableTarget<'ctx>,
        llvm_args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<CallSiteValue<'ctx>, LlvmEmitError> {
        self.emit_native_callable_call_impl(at, abi, target, llvm_args)
    }

    pub(in crate::llvm::codegen) fn callable_uses_explicit_effect_hidden_abi(
        &self,
        callable_fqn: &str,
    ) -> bool {
        self.callable_uses_explicit_effect_hidden_abi_impl(callable_fqn)
    }

    pub(in crate::llvm::codegen) fn direct_call_abi_identity(
        &self,
        callable_fqn: &str,
    ) -> hir::CallableAbiIdentity {
        self.direct_call_abi_identity_impl(callable_fqn)
    }

    pub(in crate::llvm::codegen) fn managed_callable_abi_identity(
        &self,
        call_may_suspend: bool,
    ) -> hir::CallableAbiIdentity {
        self.managed_callable_abi_identity_impl(call_may_suspend)
    }

    pub(in crate::llvm::codegen) fn managed_callable_abi_identity_from_fun_ty(
        &self,
        fun_ty: &crate::ty::FunctionType,
    ) -> hir::CallableAbiIdentity {
        self.managed_callable_abi_identity_from_fun_ty_impl(fun_ty)
    }

    pub(in crate::llvm::codegen) fn callable_needs_callee_resume_shell(
        &self,
        callable_fqn: &str,
    ) -> bool {
        self.callable_needs_callee_resume_shell_impl(callable_fqn)
    }

    pub(in crate::llvm::codegen) fn published_callable_signature(
        &self,
        callable_fqn: &str,
    ) -> Option<(Vec<TypeId>, TypeId)> {
        self.published_callable_signature_impl(callable_fqn)
    }

    pub(in crate::llvm::codegen) fn explicit_effect_hidden_abi_param_count(
        &self,
        uses_explicit_effect_hidden_abi: bool,
    ) -> u32 {
        self.explicit_effect_hidden_abi_param_count_impl(uses_explicit_effect_hidden_abi)
    }

    pub(in crate::llvm::codegen) fn push_explicit_effect_hidden_abi_param_tys(
        &self,
        llvm_params: &mut Vec<BasicMetadataTypeEnum<'ctx>>,
    ) {
        self.push_explicit_effect_hidden_abi_param_tys_impl(llvm_params)
    }

    pub(in crate::llvm::codegen) fn bind_explicit_effect_hidden_abi_slots(
        &mut self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        first_hidden_param_index: u32,
        uses_explicit_effect_hidden_abi: bool,
    ) -> Result<(), LlvmEmitError> {
        self.bind_explicit_effect_hidden_abi_slots_impl(
            at,
            llvm_fun,
            first_hidden_param_index,
            uses_explicit_effect_hidden_abi,
        )
    }

    pub(in crate::llvm::codegen) fn clear_explicit_effect_hidden_abi_slots(&mut self) {
        self.clear_explicit_effect_hidden_abi_slots_impl()
    }

    #[allow(dead_code)]
    pub(in crate::llvm::codegen) fn build_ordinary_callee_suspend_plan(
        &self,
        body: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Option<CalleeSuspendPlan> {
        self.build_ordinary_callee_suspend_plan_impl(body, declared_return_ty)
    }

    pub(in crate::llvm::codegen) fn hir_ty_declared_effectful(
        &self,
        hir_ty: Option<TypeId>,
    ) -> bool {
        self.hir_ty_declared_effectful_impl(hir_ty)
    }

    pub(in crate::llvm::codegen) fn local_call_may_suspend_from_hir_ty(
        &self,
        hir_ty: Option<TypeId>,
    ) -> bool {
        self.local_call_may_suspend_from_hir_ty_impl(hir_ty)
    }

    pub(in crate::llvm::codegen) fn known_fun_body_may_outward_effect(
        &self,
        fqn: &str,
        declared_fun_ty: TypeId,
    ) -> bool {
        self.known_fun_body_may_outward_effect_impl(fqn, declared_fun_ty)
    }

    pub(in crate::llvm::codegen) fn function_value_expr_body_may_outward_effect_when_called_for_local(
        &self,
        expr: &hir::Expr,
    ) -> bool {
        self.function_value_expr_body_may_outward_effect_when_called_for_local_impl(expr)
    }

    pub(in crate::llvm::codegen) fn type_contains_gc_refs(
        &self,
        ty: TypeId,
        visiting: &mut HashSet<TypeId>,
    ) -> bool {
        if !visiting.insert(ty) {
            return false;
        }

        let contains = match self.types.kind(ty) {
            TypeKind::Ref(_) => true,
            TypeKind::StarProjection(star) => self.type_contains_gc_refs(star.read_ty, visiting),
            TypeKind::Param(_) => true,
            TypeKind::Value(kind) => match kind {
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_) => false,
                ValueTypeKind::Option(inner) => self.type_contains_gc_refs(*inner, visiting),
                ValueTypeKind::Tuple(elements) => elements
                    .iter()
                    .copied()
                    .any(|elem| self.type_contains_gc_refs(elem, visiting)),
                ValueTypeKind::Nominal(nominal) => {
                    let key = self.nominal_layout_key(nominal);
                    if let Some(layout) = self.struct_layouts.get(&key) {
                        layout.fields.iter().any(|field| {
                            field.ty.is_some_and(|field_ty| {
                                self.type_contains_gc_refs(field_ty, visiting)
                            })
                        })
                    } else if let Some(layout) = self.enum_layouts.get(&key) {
                        layout.variants.iter().any(|variant| {
                            variant.fields.iter().any(|field| {
                                field.ty.is_some_and(|field_ty| {
                                    self.type_contains_gc_refs(field_ty, visiting)
                                })
                            })
                        })
                    } else {
                        false
                    }
                }
            },
        };

        visiting.remove(&ty);
        contains
    }

    pub(in crate::llvm::codegen) fn cg_value_from_llvm_param(
        &self,
        at: crate::span::Span,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        target_ty: CgTy,
        missing_kind: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.cg_value_from_llvm_param_impl(at, llvm_fun, param_index, target_ty, missing_kind)
    }

    pub(in crate::llvm::codegen) fn bind_ordinary_param_local(
        &mut self,
        binding: OrdinaryParamLocalBinding<'ctx, '_>,
    ) -> Result<(), LlvmEmitError> {
        self.bind_ordinary_param_local_impl(binding)
    }

    pub(in crate::llvm::codegen) fn materialize_deferred_cg_value_for_call_arg(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<(CgValue<'ctx>, Vec<DeferredGcSensitiveSpill<'ctx>>), LlvmEmitError> {
        self.materialize_deferred_cg_value_for_call_arg_impl(at, name, value)
    }

    pub(in crate::llvm::codegen) fn deferred_gc_spill_slot_for_call_arg(
        &mut self,
        at: crate::span::Span,
        name: &str,
        value: DeferredCgValue<'ctx>,
    ) -> Result<(PointerValue<'ctx>, Vec<DeferredGcSensitiveSpill<'ctx>>), LlvmEmitError> {
        self.deferred_gc_spill_slot_for_call_arg_impl(at, name, value)
    }

    pub(in crate::llvm::codegen) fn release_evaluated_call_arg_roots(
        &mut self,
        args: &[EvaluatedCallArg<'ctx>],
    ) {
        self.release_evaluated_call_arg_roots_impl(args)
    }

    pub(in crate::llvm::codegen) fn as_llvm_arg_value(
        &self,
        span: crate::span::Span,
        param_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<inkwell::values::BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        self.as_llvm_arg_value_impl(span, param_ty, value)
    }

    #[allow(dead_code)]
    pub(in crate::llvm::codegen) fn codegen_fun_params(
        &mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
        param_offset: u32,
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in fun.params.iter().enumerate() {
            self.bind_ordinary_param_local(OrdinaryParamLocalBinding {
                at: param.span,
                llvm_fun,
                param_index: idx as u32 + param_offset,
                name: &param.name,
                id: param.id,
                ty_id: param.ty,
                call_may_suspend: self.local_call_may_suspend_from_hir_ty(Some(param.ty)),
                missing_kind: "missing llvm param",
            })?;
        }
        Ok(())
    }

    pub(in crate::llvm::codegen) fn default_value(
        &mut self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match ty {
            CgTy::Unit => Ok(CgValue::unit()),
            // T1612: Nothing/Never has no runtime value.
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let llvm_ty = self.llvm_basic_type_of(at, ty)?;
                let raw = self.zero_initializer_for_basic_type(llvm_ty);
                self.cg_value_from_loaded(at, ty, raw)
            }
        }
    }

    pub(in crate::llvm::codegen) fn declare_libc_exit(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("exit") {
            return f;
        }
        let fn_ty = self
            .context
            .void_type()
            .fn_type(&[self.context.i32_type().into()], false);
        self.declare_runtime_or_native_import_function("exit", fn_ty)
    }

    pub(in crate::llvm::codegen) fn declare_libc_malloc(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("malloc") {
            return f;
        }

        // `void* malloc(size_t size)`：这里用 `i64` 作为 size（host 64-bit 场景；32-bit 下会被 truncate）。
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let size_ty = self.context.i64_type();
        let fn_ty = i8_ptr_ty.fn_type(&[size_ty.into()], false);
        self.declare_runtime_or_native_import_function("malloc", fn_ty)
    }

    pub(in crate::llvm::codegen) fn emit_exit_with_code(
        &mut self,
        at: crate::span::Span,
        code: i32,
    ) -> Result<(), LlvmEmitError> {
        let exit = self.declare_libc_exit();
        let code_i32 = self.context.i32_type().const_int(code as u64, false);
        let _ = self.builder.build_call(exit, &[code_i32.into()], "exit")?;
        self.builder.build_unreachable()?;
        let _ = at;
        Ok(())
    }

    /// T0141: Codegen an early `return` from inside a nested block or loop.
    /// Stores the return value into the function-level return alloca and branches to the return BB.
    pub(in crate::llvm::codegen) fn codegen_early_return(
        &mut self,
        _span: crate::span::Span,
        value: Option<&hir::Expr>,
    ) -> Result<(), LlvmEmitError> {
        let return_ctx = self.function_cx.return_context.unwrap_or_else(|| {
            unreachable!(
                "typecheck must reject `return` outside function bodies before LLVM codegen"
            )
        });
        let declared_return_cg = self.function_cx.current_fun_return_ty.unwrap_or(CgTy::Unit);

        match value {
            Some(expr) => {
                let v = self.codegen_expr_in_expected_context(expr, Some(declared_return_cg))?;
                if let Some(alloca) = return_ctx.return_alloca {
                    let coerced = self.coerce_value(expr.span, v, declared_return_cg)?;
                    if let Some(raw) = coerced.value {
                        self.builder.build_store(alloca, raw)?;
                    }
                }
            }
            None => {
                // `return` without value — for Unit functions, no store needed.
            }
        }

        self.builder
            .build_unconditional_branch(return_ctx.return_bb)?;
        Ok(())
    }

    pub(in crate::llvm::codegen) fn classify_native_callable_body_symbol(
        &mut self,
        span: crate::span::Span,
        param_tys: &[TypeId],
        return_ty: TypeId,
        calling_convention: &str,
    ) -> Result<NativeCallableAbi<'ctx>, LlvmEmitError> {
        self.classify_native_callable_body_symbol_impl(
            span,
            param_tys,
            return_ty,
            calling_convention,
        )
    }
}
