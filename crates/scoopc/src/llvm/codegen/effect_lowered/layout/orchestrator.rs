//! Top-level orchestration of program ABI materialization.
//!
//! Holds the entry points that drive every other submodule: `new` performs
//! initial validation and constructs the materializer, while `materialize`
//! runs the published phases in order — state-machine layouts, callable
//! layouts, surface-resume publication, carrier shells, boundary contracts,
//! payload bindings, dynamic-invoke layouts, and handle dispatch.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn new(
        codegen: &'cg mut MainCodegen<'a, 'ctx>,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a crate::mir::MaterializedMirPassView<'a>,
        effect_facts: &'a MaterializedEffectFacts,
    ) -> Result<Self, LlvmEmitError> {
        validate_program_layout_inventory(program)?;
        Ok(Self {
            codegen,
            program,
            source_types,
            pass_view,
            effect_facts,
            source_value_layouts: BTreeMap::new(),
        })
    }

    pub(super) fn materialize(self) -> Result<ProgramAbiQuery<'ctx>, LlvmEmitError> {
        let mut this = self;
        let mut step_layouts = BTreeMap::new();
        for step_type in this.program.step_types() {
            step_layouts.insert(
                step_type.step_schema(),
                this.materialize_step_layout(step_type)?,
            );
        }

        let mut resume_packing_layouts = BTreeMap::new();
        for interface in this.program.resume_packings() {
            resume_packing_layouts.insert(
                interface.interface_id(),
                this.materialize_resume_packing_layout(interface, &step_layouts)?,
            );
        }

        let surface_resume_layouts = this.materialize_surface_resume_layouts(&step_layouts)?;
        this.validate_resume_site_surface_contracts(&surface_resume_layouts)?;

        let mut frame_layouts = BTreeMap::new();
        for callable in this.program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            frame_layouts.insert(
                callable.step_schema(),
                this.materialize_frame_layout(callable)?,
            );
        }

        let mut continuation_layouts = BTreeMap::new();
        for object in this.program.continuation_objects() {
            continuation_layouts.insert(
                object.object_id(),
                this.materialize_continuation_object_layout(object, &surface_resume_layouts)?,
            );
        }

        let mut callable_layouts = BTreeMap::new();
        for callable in this.program.callables() {
            if callable.effect_step_abi().is_none() {
                continue;
            }
            callable_layouts.insert(
                callable.step_schema(),
                this.materialize_callable_layout(callable, &step_layouts)?,
            );
        }
        let mut plain_callable_layouts_by_version_key = HashMap::new();
        for callable in this.program.callables() {
            if callable.plain_abi().is_none() {
                continue;
            }
            let layout = this.materialize_plain_callable_layout(callable)?;
            if let Some(existing) = plain_callable_layouts_by_version_key
                .insert(callable.body_version_key().clone(), layout)
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 body version key {:?} 同时发布了多个 plain callable layout（已有 `{}`，新值 `{}`）",
                    callable.body_version_key(),
                    existing.root_fqn(),
                    callable.root_fqn(),
                )));
            }
        }
        let callable_layouts_by_version_key =
            this.materialize_callable_version_layout_index(&callable_layouts)?;
        let mut plain_local_effect_step_schemas_by_version_key = HashMap::new();
        for callable in this.program.callables() {
            let Some(local) = callable
                .plain_abi()
                .and_then(|plain| plain.local_effect_control())
            else {
                continue;
            };
            plain_local_effect_step_schemas_by_version_key
                .insert(callable.body_version_key().clone(), local.step_schema());
        }
        let known_instance_callable_versions =
            this.materialize_known_instance_callable_versions(&callable_layouts)?;

        let surface_resume_dispatch_layouts = this.materialize_surface_resume_dispatch_layouts(
            &surface_resume_layouts,
            &continuation_layouts,
            &resume_packing_layouts,
            &callable_layouts,
            &frame_layouts,
        )?;

        let dynamic_invoke_layouts = this.materialize_dynamic_invoke_layouts(&step_layouts)?;
        let callable_carrier_target_layouts = this.publish_callable_carrier_entry_shells(
            &callable_layouts,
            &step_layouts,
            &dynamic_invoke_layouts,
        )?;
        let (
            call_boundary_operand_layouts,
            perform_boundary_operand_layouts,
            resume_boundary_operand_layouts,
        ) = this.materialize_boundary_operand_layouts(
            &dynamic_invoke_layouts,
            &surface_resume_layouts,
            &surface_resume_dispatch_layouts,
        )?;
        let (resume_payload_binding_layouts, resume_payload_bindings_by_state) =
            this.materialize_resume_payload_binding_layouts(&frame_layouts)?;
        let completion_payload_binding_layouts =
            this.materialize_completion_payload_binding_layouts(&step_layouts, &frame_layouts)?;
        let local_runtime_error_contracts = this.materialize_local_runtime_error_contracts()?;
        let handle_dispatch_layouts = this.materialize_handle_dispatch_layouts(
            &frame_layouts,
            &continuation_layouts,
            &surface_resume_layouts,
        )?;
        this.validate_source_statement_classifications()?;
        let class_instance_layouts = this.materialize_class_instance_layouts()?;

        Ok(ProgramAbiQuery::new(
            this.source_value_layouts,
            class_instance_layouts,
            step_layouts,
            frame_layouts,
            continuation_layouts,
            resume_packing_layouts,
            surface_resume_layouts,
            surface_resume_dispatch_layouts,
            callable_layouts,
            callable_layouts_by_version_key,
            plain_local_effect_step_schemas_by_version_key,
            plain_callable_layouts_by_version_key,
            known_instance_callable_versions,
            callable_carrier_target_layouts,
            dynamic_invoke_layouts,
            call_boundary_operand_layouts,
            perform_boundary_operand_layouts,
            resume_boundary_operand_layouts,
            resume_payload_binding_layouts,
            resume_payload_bindings_by_state,
            completion_payload_binding_layouts,
            local_runtime_error_contracts,
            handle_dispatch_layouts,
        ))
    }
}
