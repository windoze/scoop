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
}
