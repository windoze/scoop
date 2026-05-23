//! Callable carrier entry-shell publishing and ABI wrapping.
//!
//! Carriers are the dispatchable shells for closure / virtual / interface
//! callables. This module publishes one entry shell per published callable
//! family, registers their target contracts, and provides ABI helpers that
//! pack closure environments and dispatch receivers into the canonical tuple
//! shape.

use super::*;

impl<'cg, 'a, 'ctx> ProgramAbiMaterializer<'cg, 'a, 'ctx> {
    pub(super) fn publish_callable_carrier_entry_shells(
        &mut self,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        dynamic_invoke_layouts: &BTreeMap<(StepSchemaId, SiteId), DynamicInvokeLayout<'ctx>>,
    ) -> Result<HashMap<(CallableCarrierKind, String), CallableCarrierTargetLayout>, LlvmEmitError>
    {
        let published_callable_roots = self
            .program
            .callables()
            .iter()
            .filter(|callable| callable.effect_step_abi().is_some())
            .map(|callable| callable.root_fqn())
            .collect::<BTreeSet<_>>();
        let plain_callable_roots = self
            .program
            .callables()
            .iter()
            .filter(|callable| callable.plain_abi().is_some())
            .map(|callable| callable.root_fqn())
            .collect::<BTreeSet<_>>();
        let closure_targets = published_callable_roots.clone();
        let plain_closure_targets = plain_callable_roots.clone();
        // Vtable/itable carrier publication consumes the LIR physical layout inventory.
        let class_vtable_targets = self
            .lir_facts
            .physical_layout
            .class_vtables
            .values()
            .flat_map(|slots| slots.iter().map(|slot| slot.impl_member_fqn.as_str()))
            .filter(|impl_fqn| published_callable_roots.contains(impl_fqn))
            .collect::<BTreeSet<_>>();
        let plain_class_vtable_targets = self
            .lir_facts
            .physical_layout
            .class_vtables
            .values()
            .flat_map(|slots| slots.iter().map(|slot| slot.impl_member_fqn.as_str()))
            .filter(|impl_fqn| plain_callable_roots.contains(impl_fqn))
            .collect::<BTreeSet<_>>();
        let mut interface_itable_targets = self
            .lir_facts
            .physical_layout
            .class_itables
            .values()
            .flat_map(|entries| {
                entries.entries.iter().flat_map(|entry| {
                    entry
                        .method_impl_fqns
                        .iter()
                        .filter(|impl_fqn| !impl_fqn.is_empty())
                })
            })
            .filter(|impl_fqn| published_callable_roots.contains(impl_fqn.as_str()))
            .cloned()
            .collect::<BTreeSet<String>>();
        let mut plain_interface_itable_targets = self
            .lir_facts
            .physical_layout
            .class_itables
            .values()
            .flat_map(|entries| {
                entries.entries.iter().flat_map(|entry| {
                    entry
                        .method_impl_fqns
                        .iter()
                        .filter(|impl_fqn| !impl_fqn.is_empty())
                })
            })
            .filter(|impl_fqn| plain_callable_roots.contains(impl_fqn.as_str()))
            .cloned()
            .collect::<BTreeSet<String>>();

        for source_ty in self.codegen.types.iter_ids() {
            for entry in self.codegen.mir_value_box_itable_entries(source_ty)? {
                for impl_fqn in entry
                    .method_impl_fqns
                    .iter()
                    .filter(|impl_fqn| !impl_fqn.is_empty())
                {
                    if published_callable_roots.contains(impl_fqn.as_str()) {
                        interface_itable_targets.insert(impl_fqn.clone());
                    }
                    if plain_callable_roots.contains(impl_fqn.as_str()) {
                        plain_interface_itable_targets.insert(impl_fqn.clone());
                    }
                }
            }
        }

        let mut carrier_layouts = HashMap::new();
        let dynamic_dispatch_targets =
            self.dynamic_dispatch_carrier_targets(dynamic_invoke_layouts)?;
        for callable_fqn in plain_closure_targets {
            self.publish_plain_carrier_fallback_target(
                CallableCarrierKind::ClosureObject,
                callable_fqn,
            )?;
        }
        for impl_fqn in plain_class_vtable_targets {
            self.publish_plain_carrier_fallback_target(CallableCarrierKind::ClassVtable, impl_fqn)?;
        }
        for impl_fqn in plain_interface_itable_targets {
            self.publish_plain_carrier_fallback_target(
                CallableCarrierKind::InterfaceItable,
                &impl_fqn,
            )?;
        }
        for callable_fqn in closure_targets {
            self.publish_closure_carrier_entry_shell(
                callable_fqn,
                callable_layouts,
                step_layouts,
                &mut carrier_layouts,
            )?;
        }
        for impl_fqn in class_vtable_targets {
            let return_step_schema = dynamic_dispatch_targets
                .get(&(CallableCarrierKind::ClassVtable, impl_fqn.to_string()))
                .copied();
            self.publish_dispatch_carrier_entry_shell(
                CallableCarrierKind::ClassVtable,
                impl_fqn,
                return_step_schema,
                callable_layouts,
                step_layouts,
                &mut carrier_layouts,
            )?;
        }
        for impl_fqn in interface_itable_targets {
            let return_step_schema = dynamic_dispatch_targets
                .get(&(CallableCarrierKind::InterfaceItable, impl_fqn.to_string()))
                .copied();
            self.publish_dispatch_carrier_entry_shell(
                CallableCarrierKind::InterfaceItable,
                &impl_fqn,
                return_step_schema,
                callable_layouts,
                step_layouts,
                &mut carrier_layouts,
            )?;
        }

        self.codegen.enable_callable_carrier_contract();
        Ok(carrier_layouts)
    }

    pub(super) fn publish_plain_carrier_fallback_target(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<(), LlvmEmitError> {
        self.codegen
            .register_plain_callable_carrier_fallback(kind, callable_fqn)?;
        Ok(())
    }

    pub(super) fn dynamic_dispatch_carrier_targets(
        &self,
        dynamic_invoke_layouts: &BTreeMap<(StepSchemaId, SiteId), DynamicInvokeLayout<'ctx>>,
    ) -> Result<HashMap<(CallableCarrierKind, String), StepSchemaId>, LlvmEmitError> {
        let mut targets = HashMap::<(CallableCarrierKind, String), StepSchemaId>::new();
        for layout in dynamic_invoke_layouts.values() {
            let kind = match layout.carrier() {
                DynamicInvokeCarrierLayout::VirtualReceiver(_) => CallableCarrierKind::ClassVtable,
                DynamicInvokeCarrierLayout::InterfaceReceiver(_) => {
                    CallableCarrierKind::InterfaceItable
                }
                DynamicInvokeCarrierLayout::ClosureObject(_)
                | DynamicInvokeCarrierLayout::FunPtr(_) => continue,
            };
            for fqn in layout.candidate_targets() {
                let key = (kind, fqn.clone());
                if let Some(existing) = targets.insert(key.clone(), layout.return_step_schema())
                    && existing != layout.return_step_schema()
                {
                    return Err(frontend_error(format!(
                        "LLVM ABI materialization 发现 {} `{}` 需要多个 dynamic carrier return schema：s{} 与 s{}",
                        kind.label(),
                        fqn,
                        existing.as_u32(),
                        layout.return_step_schema().as_u32(),
                    )));
                }
            }
        }
        Ok(targets)
    }

    pub(super) fn publish_closure_carrier_entry_shell(
        &mut self,
        callable_fqn: &str,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        carrier_layouts: &mut HashMap<(CallableCarrierKind, String), CallableCarrierTargetLayout>,
    ) -> Result<(), LlvmEmitError> {
        let callable_layout = self.callable_layout_for_carrier_target(
            callable_layouts,
            CallableCarrierKind::ClosureObject,
            callable_fqn,
        )?;
        let step_layout = step_layouts
            .get(&callable_layout.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 callable `{}` closure carrier target 的 step layout {}",
                    callable_fqn,
                    callable_layout.step_schema().as_u32(),
                ))
            })?;
        let step_ty = step_layout.llvm_ty();
        let args_abi = self.closure_carrier_args_abi(callable_fqn)?;
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![self.codegen.llvm_gc_i8_ptr_type().into()];
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let symbol_name = stable_naming::private_name_from_key_text(
            "closure_dynamic_entry",
            step_layout.stable_effect_key_text(),
        );
        self.ensure_declared_compiler_private_helper_function(
            &symbol_name,
            step_ty.fn_type(&params, false),
        );
        self.register_callable_carrier_target_contract(
            CallableCarrierKind::ClosureObject,
            callable_fqn,
            callable_layout,
            callable_layout.step_schema(),
            &symbol_name,
            carrier_layouts,
        )?;
        Ok(())
    }

    pub(super) fn publish_dispatch_carrier_entry_shell(
        &mut self,
        kind: CallableCarrierKind,
        impl_fqn: &str,
        return_step_schema: Option<StepSchemaId>,
        callable_layouts: &BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        step_layouts: &BTreeMap<StepSchemaId, StepLayout<'ctx>>,
        carrier_layouts: &mut HashMap<(CallableCarrierKind, String), CallableCarrierTargetLayout>,
    ) -> Result<(), LlvmEmitError> {
        let callable_layout =
            self.callable_layout_for_carrier_target(callable_layouts, kind, impl_fqn)?;
        let return_step_schema = return_step_schema.unwrap_or(callable_layout.step_schema());
        let step_ty = step_layouts
            .get(&return_step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 {} `{}` target 的 step layout {}",
                    kind.label(),
                    impl_fqn,
                    return_step_schema.as_u32(),
                ))
            })?
            .llvm_ty();
        let owner_step_layout = step_layouts
            .get(&callable_layout.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "LLVM ABI materialization 缺少 {} `{}` owner step layout {}",
                    kind.label(),
                    impl_fqn,
                    callable_layout.step_schema().as_u32(),
                ))
            })?;
        let (receiver_abi, args_abi) = self.dispatch_carrier_receiver_and_args_abi(impl_fqn)?;
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> = vec![receiver_abi.llvm_ty().into()];
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let symbol_name = stable_naming::private_name_from_key_text(
            match kind {
                CallableCarrierKind::ClassVtable => "vtable_dynamic_entry",
                CallableCarrierKind::InterfaceItable => "itable_dynamic_entry",
                CallableCarrierKind::ClosureObject => "closure_dynamic_entry",
            },
            owner_step_layout.stable_effect_key_text(),
        );
        self.ensure_declared_compiler_private_helper_function(
            &symbol_name,
            step_ty.fn_type(&params, false),
        );
        self.register_callable_carrier_target_contract(
            kind,
            impl_fqn,
            callable_layout,
            return_step_schema,
            &symbol_name,
            carrier_layouts,
        )?;
        Ok(())
    }

    pub(super) fn register_callable_carrier_target_contract(
        &self,
        kind: CallableCarrierKind,
        callable_fqn: &str,
        callable_layout: &CallableLayout<'ctx>,
        return_step_schema: StepSchemaId,
        symbol_name: &str,
        carrier_layouts: &mut HashMap<(CallableCarrierKind, String), CallableCarrierTargetLayout>,
    ) -> Result<(), LlvmEmitError> {
        self.codegen
            .register_callable_carrier_entry_symbol(kind, callable_fqn, symbol_name)?;

        let key = (kind, callable_fqn.to_string());
        let published = CallableCarrierTargetLayout::new(
            callable_fqn.to_string(),
            callable_layout.body_version_key().clone(),
            return_step_schema,
            symbol_name.to_string(),
        );
        if let Some(existing) = carrier_layouts.get(&key) {
            if existing != &published {
                return Err(frontend_error(format!(
                    "LLVM ABI materialization 发现 {} `{}` 重复发布了不兼容的 callable version contract：已有 {:?}，新值 {:?}",
                    kind.label(),
                    callable_fqn,
                    existing,
                    published,
                )));
            }
            return Ok(());
        }
        carrier_layouts.insert(key, published);
        Ok(())
    }

    pub(super) fn callable_layout_for_carrier_target<'b>(
        &self,
        callable_layouts: &'b BTreeMap<StepSchemaId, CallableLayout<'ctx>>,
        kind: CallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<&'b CallableLayout<'ctx>, LlvmEmitError> {
        let matches = callable_layouts
            .values()
            .filter(|layout| layout.root_fqn() == callable_fqn)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(frontend_error(format!(
                "LLVM ABI materialization 缺少 {} `{}` 的 published callable version，无法发布 carrier target",
                kind.label(),
                callable_fqn,
            ))),
            [layout] => Ok(*layout),
            _ => Err(frontend_error(format!(
                "LLVM ABI materialization 发现 {} `{}` 存在多个 published callable version {:?}，缺少 authoritative version selector，无法发布 carrier target",
                kind.label(),
                callable_fqn,
                matches
                    .iter()
                    .map(|layout| layout.body_version_key())
                    .collect::<Vec<_>>(),
            ))),
        }
    }

    pub(super) fn closure_carrier_args_abi(
        &mut self,
        root_fqn: &str,
    ) -> Result<AbiValue<'ctx>, LlvmEmitError> {
        let facts = self.effect_step_callable_facts_for_root(root_fqn)?;
        let component_tys = facts.closure_carrier_arg_tys.clone();
        self.canonical_tuple_abi_from_types(self.source_types, &component_tys)
    }

    pub(super) fn dispatch_carrier_receiver_and_args_abi(
        &mut self,
        impl_fqn: &str,
    ) -> Result<(AbiValue<'ctx>, AbiValue<'ctx>), LlvmEmitError> {
        let facts = self.effect_step_callable_facts_for_root(impl_fqn)?;
        let param_tys = facts.param_tys.clone();
        let Some((receiver, explicit_params)) = param_tys.split_first() else {
            return Err(frontend_error(format!(
                "LLVM ABI materialization 发现 dispatch target `{impl_fqn}` 没有 receiver 参数，无法发布 vtable/itable carrier target"
            )));
        };
        let receiver = *receiver;
        let args = explicit_params.to_vec();
        Ok((
            self.abi_value_from_types(self.source_types, receiver)?,
            self.canonical_tuple_abi_from_types(self.source_types, &args)?,
        ))
    }

    pub(super) fn canonical_tuple_abi_from_types(
        &mut self,
        types: &TypeStore,
        components: &[TypeId],
    ) -> Result<AbiValue<'ctx>, LlvmEmitError> {
        match components {
            [] => Ok(AbiValue::new(
                self.codegen.context.struct_type(&[], false).into(),
                true,
            )),
            [single] => self
                .abi_value_from_types(types, *single)
                .map_err(|err| self.wrap_tuple_abi_error(types, components, 0, *single, err)),
            _ => {
                let mut fields = Vec::with_capacity(components.len());
                for (index, component) in components.iter().copied().enumerate() {
                    let llvm_ty = self
                        .llvm_abi_type_of_types(types, component)
                        .map_err(|err| {
                            self.wrap_tuple_abi_error(types, components, index, component, err)
                        })?;
                    if self.codegen.target_data.get_store_size(&llvm_ty) == 0 {
                        continue;
                    }
                    fields.push(llvm_ty);
                }
                let llvm_ty = self.codegen.context.struct_type(&fields, false).into();
                Ok(AbiValue::new(
                    llvm_ty,
                    self.codegen.target_data.get_store_size(&llvm_ty) == 0,
                ))
            }
        }
    }

    pub(super) fn wrap_tuple_abi_error(
        &self,
        types: &TypeStore,
        components: &[TypeId],
        index: usize,
        component: TypeId,
        err: LlvmEmitError,
    ) -> LlvmEmitError {
        match err {
            LlvmEmitError::Frontend { message } => {
                let component_list = components
                    .iter()
                    .map(|ty| types.display(*ty).to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                frontend_error(format!(
                    "LLVM tuple ABI component #{index} `{}`（t{}）lowering failed in ({component_list}): {message}",
                    types.display(component),
                    component.as_u32()
                ))
            }
            other => other,
        }
    }
}

pub(super) fn expected_source_types_for_carrier(
    types: &TypeStore,
    carrier_ty: TypeId,
    source_count: usize,
) -> Result<Vec<TypeId>, String> {
    match source_count {
        0 => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Unit) => Ok(Vec::new()),
            _ => Err(format!(
                "只有 Unit carrier 才允许 0 个 source，但 published carrier 为 t{}",
                carrier_ty.as_u32(),
            )),
        },
        1 => Ok(vec![carrier_ty]),
        _ => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) if elements.len() == source_count => {
                Ok(elements.clone())
            }
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => Err(format!(
                "published tuple carrier t{} 期望 {} 个 source，实际为 {source_count}",
                carrier_ty.as_u32(),
                elements.len(),
            )),
            _ => Err(format!(
                "published carrier t{} 期望单一 source，实际数量为 {source_count}",
                carrier_ty.as_u32(),
            )),
        },
    }
}
