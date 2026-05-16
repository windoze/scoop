//! Call and invoke lowering: known-instance call steps, dynamic-invoke step emission, dynamic-call-carrier loads, and the operand-source / call-arg packing helpers used by both paths.

use super::*;

impl<'cg, 'a, 'ctx> RefactorCallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn lower_operand_source(
        &mut self,
        source: &LateLoweredOperandSource,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives().lower_operand_source(source)
    }

    pub(super) fn pack_sources(
        &mut self,
        source_ty: TypeId,
        sources: &[LateLoweredOperandSource],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.value_primitives()
            .pack_sources(source_ty, sources, name)
    }

    pub(super) fn pack_call_args_for_invoke(
        &mut self,
        span: crate::span::Span,
        invoke_args_tuple_ty: TypeId,
        args: &[mir::CallArg],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.value_primitives()
            .pack_call_args_for_invoke_args_tuple(span, invoke_args_tuple_ty, args, name)
    }

    pub(super) fn emit_known_instance_call_step(
        &mut self,
        site_id: SiteId,
        entry: &RefactorCallableEntryLayout<'ctx>,
        args_payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let callee = self.codegen.refactor_function(entry.symbol_name())?;
        let mut args = Vec::new();
        if !entry.args_abi().is_elided() {
            args.push(
                args_payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor call site {} 需要 non-elided args payload",
                            site_id.as_u32()
                        ))
                    })?
                    .into(),
            );
        }
        let call = self.codegen.build_call_preserving_gc_local_roots(
            self.mir_fun.span,
            callee,
            &args,
            "refactor_call_step",
        )?;
        call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error("refactor call boundary callee 未返回 Step_F".to_string())
        })
    }

    pub(super) fn body_operand_source_ty(&self, operand: &crate::mir::Operand) -> Option<TypeId> {
        match operand {
            crate::mir::Operand::Local(local) => self
                .body
                .locals
                .get(local.as_u32() as usize)
                .map(|decl| decl.ty),
            crate::mir::Operand::Const(_) => None,
        }
    }

    pub(super) fn lower_dynamic_call_carrier(
        &mut self,
        span: crate::span::Span,
        kind: &mir::CallKind,
        layout: &RefactorDynamicInvokeLayout<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let (operand, expected_ty) = match (kind, layout.carrier()) {
            (
                mir::CallKind::Closure { callee, .. } | mir::CallKind::FunValue { callee },
                RefactorDynamicInvokeCarrierLayout::ClosureObject(_),
            ) => (callee, CgTy::Ref),
            (mir::CallKind::FunPtr { callee }, RefactorDynamicInvokeCarrierLayout::FunPtr(_)) => {
                let source_ty = self.body_operand_source_ty(callee).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor funptr carrier source type",
                        at: span.into(),
                    },
                )?;
                let expected = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, source_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor funptr carrier cg type",
                        at: span.into(),
                    })?;
                (callee, expected)
            }
            (
                mir::CallKind::Virtual { receiver, .. },
                RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch),
            ) => {
                let expected = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, dispatch.receiver_ty())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor virtual receiver type",
                        at: span.into(),
                    })?;
                (receiver, expected)
            }
            (
                mir::CallKind::Interface { receiver, .. },
                RefactorDynamicInvokeCarrierLayout::InterfaceReceiver(dispatch),
            ) => {
                let expected = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, dispatch.receiver_ty())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor interface receiver type",
                        at: span.into(),
                    })?;
                (receiver, expected)
            }
            _ => {
                return Err(frontend_error(format!(
                    "refactor dynamic call site {} 的 CallKind 与 published carrier layout 漂移",
                    layout.site_id().as_u32()
                )));
            }
        };
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            operand,
            &self.slots,
            Some(expected_ty),
        )?;
        let value = self.codegen.coerce_value(span, value, expected_ty)?;
        value.value.ok_or_else(|| {
            frontend_error(format!(
                "refactor dynamic call site {} carrier source 缺少可传递值",
                layout.site_id().as_u32()
            ))
        })
    }

    pub(super) fn emit_refactor_dynamic_invoke_step(
        &mut self,
        layout: &RefactorDynamicInvokeLayout<'ctx>,
        carrier: BasicValueEnum<'ctx>,
        args_payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let fn_i8 = self.load_dynamic_invoke_fn_ptr(layout, carrier)?;
        let typed_fn = self.codegen.refactor_cast_ptr(
            fn_i8,
            self.codegen.context.ptr_type(AddressSpace::default()),
            "refactor_dynamic_fn",
        )?;
        let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        args.push(carrier.into());
        if !layout.args_abi().is_elided() {
            args.push(
                args_payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor dynamic call site {} 需要 non-elided args payload",
                            layout.site_id().as_u32()
                        ))
                    })?
                    .into(),
            );
        }
        let call =
            self.codegen
                .with_conservative_gc_local_root_spills(self.mir_fun.span, |codegen| {
                    Ok(codegen.builder.build_indirect_call(
                        layout.llvm_ty(),
                        typed_fn,
                        &args,
                        "refactor_dynamic_call_step",
                    )?)
                })?;
        call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "refactor dynamic call site {} 未返回 Step_F",
                layout.site_id().as_u32()
            ))
        })
    }

    pub(super) fn load_dynamic_invoke_fn_ptr(
        &mut self,
        layout: &RefactorDynamicInvokeLayout<'ctx>,
        carrier: BasicValueEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        match layout.carrier() {
            RefactorDynamicInvokeCarrierLayout::ClosureObject(closure) => {
                let BasicValueEnum::PointerValue(carrier) = carrier else {
                    return Err(frontend_error(format!(
                        "refactor dynamic call site {} closure carrier source 不是 pointer",
                        layout.site_id().as_u32()
                    )));
                };
                if closure.fn_field_index() >= closure.object_ty().count_fields() {
                    return Err(frontend_error(format!(
                        "refactor dynamic closure carrier site {} fn field {} 越界（field_count={}）",
                        layout.site_id().as_u32(),
                        closure.fn_field_index(),
                        closure.object_ty().count_fields(),
                    )));
                }
                let obj_ptr = self.codegen.refactor_cast_ptr(
                    carrier,
                    self.codegen
                        .context
                        .ptr_type(self.codegen.gc_address_space()),
                    "refactor_dynamic_closure_obj",
                )?;
                let fn_gep = self.codegen.builder.build_struct_gep(
                    closure.object_ty(),
                    obj_ptr,
                    closure.fn_field_index(),
                    "refactor_dynamic_closure_fn_gep",
                )?;
                Ok(self
                    .codegen
                    .builder
                    .build_load(
                        self.codegen.llvm_i8_ptr_type(),
                        fn_gep,
                        "refactor_dynamic_closure_fn",
                    )?
                    .into_pointer_value())
            }
            RefactorDynamicInvokeCarrierLayout::FunPtr(_) => {
                let BasicValueEnum::IntValue(funptr_addr) = carrier else {
                    return Err(frontend_error(format!(
                        "refactor dynamic call site {} funptr carrier source 不是 machine word",
                        layout.site_id().as_u32()
                    )));
                };
                Ok(self.codegen.builder.build_int_to_ptr(
                    funptr_addr,
                    self.codegen.llvm_i8_ptr_type(),
                    "refactor_dynamic_funptr_fn",
                )?)
            }
            RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch) => {
                let BasicValueEnum::PointerValue(carrier) = carrier else {
                    return Err(frontend_error(format!(
                        "refactor dynamic call site {} virtual receiver 不是 pointer",
                        layout.site_id().as_u32()
                    )));
                };
                self.codegen.load_class_vtable_slot_fn_ptr_i8(
                    self.mir_fun.span,
                    carrier,
                    dispatch.method_slot(),
                )
            }
            RefactorDynamicInvokeCarrierLayout::InterfaceReceiver(dispatch) => {
                let BasicValueEnum::PointerValue(carrier) = carrier else {
                    return Err(frontend_error(format!(
                        "refactor dynamic call site {} interface receiver 不是 pointer",
                        layout.site_id().as_u32()
                    )));
                };
                let interface_id = dispatch.interface_id().ok_or_else(|| {
                    frontend_error(format!(
                        "refactor dynamic interface call site {} 缺少 published interface id",
                        layout.site_id().as_u32()
                    ))
                })?;
                let fn_i8 = self.codegen.load_interface_itable_slot_fn_ptr_i8(
                    self.mir_fun.span,
                    carrier,
                    interface_id,
                    dispatch.method_slot(),
                )?;
                let is_null = self
                    .codegen
                    .builder
                    .build_is_null(fn_i8, "refactor_dynamic_itable_fn_is_null")?;
                let function = self.function;
                let ok_bb = self
                    .codegen
                    .context
                    .append_basic_block(function, "refactor_dynamic_itable_ok");
                let bad_bb = self
                    .codegen
                    .context
                    .append_basic_block(function, "refactor_dynamic_itable_null");
                self.codegen
                    .builder
                    .build_conditional_branch(is_null, bad_bb, ok_bb)?;
                self.codegen.builder.position_at_end(bad_bb);
                let exit = self.codegen.declare_libc_exit();
                let code = self.codegen.context.i32_type().const_int(7, false);
                self.codegen.builder.build_call(
                    exit,
                    &[code.into()],
                    "refactor_dynamic_itable_null_exit",
                )?;
                self.codegen.builder.build_unreachable()?;
                self.codegen.builder.position_at_end(ok_bb);
                Ok(fn_i8)
            }
        }
    }
}
