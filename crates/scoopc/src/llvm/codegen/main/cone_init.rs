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

        // P5-T04 only establishes the stable per-cone skeleton. P9 lowers these
        // collected roots into eager top-level initialization inside this body.
        let _cone_identity = (plan.cone.id, plan.cone.kind);
        let _collected_root_count = plan
            .roots
            .iter()
            .filter(|root| match root.kind {
                ConeInitRootKind::TopLevelImmutableValue | ConeInitRootKind::TopLevelVar => {
                    !root.fqn.is_empty()
                }
            })
            .count();

        self.builder.build_return(None)?;
        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }
        Ok(llvm_fun)
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
