//! Per-cone initialization routine stubs and final entry calls.

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn ensure_cone_init_routines_defined(
        &mut self,
        plans: &[ConeInitRoutinePlan],
    ) -> Result<Vec<FunctionValue<'ctx>>, LlvmEmitError> {
        plans
            .iter()
            .map(|plan| self.ensure_cone_init_routine_defined(plan))
            .collect()
    }

    pub(crate) fn ensure_thread_local_init_routines_defined(
        &mut self,
        plans: &[ConeInitRoutinePlan],
    ) -> Result<Vec<FunctionValue<'ctx>>, LlvmEmitError> {
        plans
            .iter()
            .map(|plan| self.ensure_cone_init_routine_defined(plan))
            .collect()
    }

    pub(crate) fn ensure_thread_init_current_function_defined(
        &mut self,
        routines: &[FunctionValue<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        if routines.is_empty() {
            return Ok(());
        }

        let fn_ty = self.context.void_type().fn_type(&[], false);
        let llvm_fun =
            declare_exported_abi_function(self.module, "scoop_thread_init_current", fn_ty);
        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(());
        }

        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(llvm_fun, "entry");
        let init_bb = self.context.append_basic_block(llvm_fun, "init");
        let done_bb = self.context.append_basic_block(llvm_fun, "done");

        self.builder.position_at_end(entry);
        let guard = self.declare_thread_init_current_guard();
        let i8_ty = self.context.i8_type();
        let initialized = self
            .builder
            .build_load(i8_ty, guard.as_pointer_value(), "thread_init_done")?
            .into_int_value();
        let should_skip = self.builder.build_int_compare(
            IntPredicate::NE,
            initialized,
            i8_ty.const_zero(),
            "thread_init_should_skip",
        )?;
        self.builder
            .build_conditional_branch(should_skip, done_bb, init_bb)?;

        self.builder.position_at_end(init_bb);
        for routine in routines {
            let _ = self
                .builder
                .build_call(*routine, &[], "scoop_thread_local_init")?;
        }
        self.builder
            .build_store(guard.as_pointer_value(), i8_ty.const_int(1, false))?;
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        self.builder.build_return(None)?;
        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }
        Ok(())
    }

    fn ensure_cone_init_routine_defined(
        &mut self,
        plan: &ConeInitRoutinePlan,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let fn_ty = self.context.void_type().fn_type(&[], false);
        let llvm_fun = self.declare_compiler_private_helper_function(
            &plan.function_name,
            fn_ty,
            Linkage::Internal,
        );
        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);

        self.begin_function_explicit_frame_layout(llvm_fun)?;
        self.function_cx.current_fun_return_ty = Some(CgTy::Unit);
        for root in &plan.roots {
            self.emit_cone_init_root(root)?;
        }

        self.builder.build_return(None)?;
        self.finish_function_explicit_frame_layout(crate::span::Span::synthetic_prelude())?;
        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }
        Ok(llvm_fun)
    }

    fn emit_cone_init_root(&mut self, root: &ConeInitRoot) -> Result<(), LlvmEmitError> {
        match root.kind {
            ConeInitRootKind::TopLevelImmutableValue => {
                let value = self
                    .top_level_immutable_values
                    .get(&root.fqn)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "emit_cone_init_root: LIR facts accepted missing top-level immutable root `{}`",
                            root.fqn
                        )
                    });
                let init_fn =
                    self.ensure_top_level_immutable_value_init_function_defined(&value.fqn)?;
                self.with_conservative_gc_local_root_spills(value.span, |cg| {
                    let _ = cg
                        .builder
                        .build_call(init_fn, &[], "top_level_val_eager_init")?;
                    Ok(())
                })
            }
            ConeInitRootKind::TopLevelVar => {
                let var = self.top_level_vars.get(&root.fqn).cloned().unwrap_or_else(|| {
                    panic!(
                        "emit_cone_init_root: LIR facts accepted missing top-level var root `{}`",
                        root.fqn
                    )
                });
                self.emit_top_level_var_eager_initializer(&var)
            }
        }
    }

    pub(crate) fn emit_cone_init_calls(
        &mut self,
        routines: &[FunctionValue<'ctx>],
    ) -> Result<(), LlvmEmitError> {
        for routine in routines {
            let _ = self.builder.build_call(*routine, &[], "scoop_cone_init")?;
        }
        Ok(())
    }

    fn declare_thread_init_current_guard(&self) -> GlobalValue<'ctx> {
        const NAME: &str = "__scoop_priv0__thread_init_current_done";
        if let Some(existing) = self.module.get_global(NAME) {
            return existing;
        }
        let gv = self.module.add_global(self.context.i8_type(), None, NAME);
        gv.set_linkage(Linkage::Internal);
        gv.set_thread_local(true);
        gv.set_initializer(&self.context.i8_type().const_zero());
        gv
    }
}
