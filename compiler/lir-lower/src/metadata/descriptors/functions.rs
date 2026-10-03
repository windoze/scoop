use super::*;

pub(super) fn materialize(
    context: &LoweringContext,
    roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    descriptors: &mut Arena<lir::TypeDescriptor>,
    refs: &mut TypeDescriptorRefs,
) -> StorageResult<()> {
    for function in required_function_types(module) {
        let ty = mir::Type::Function(function);
        let runtime_type = runtime_type(module, &ty);
        let root = roots.for_type(&ty);
        let identity = lir::TypeDescriptorIdentity::new(runtime_type, root.clone())
            .expect("an exact function shape derives its structural descriptor identity");
        let instance_layout = lir::LayoutIdentity::managed_object(
            runtime_type.exact_type(),
            context.target_profile(),
            root,
        )
        .expect("an exact function shape derives its abstract instance layout");
        let vtable = lir::VtableRecord::new(&identity, Vec::new())
            .expect("an exact function shape derives its empty vtable identity");
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            relations: lir::TypeDescriptorRelations::Signature {
                is_suspend: module.function_types[function].is_suspend,
                parameters: module.function_types[function]
                    .parameter_types
                    .iter()
                    .map(|ty| function_operand(refs, ty))
                    .collect(),
                result: function_operand(refs, &module.function_types[function].return_type),
            },
            diagnostic_name: mir::type_name(module, &ty),
            identity,
            instance_layout,
            instance_shape: lir::TypeInstanceShapeV1::abstract_ref(),
            inline_scan: lir::TypeDescriptorInlineScanV1::Null,
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        refs.functions
            .insert(function, lir::TypeDescriptorRef::Local(descriptor));
    }
    for (closure, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(context, module, enums, def)?;
        let location = mir::GeneratedExactTypeLocation::Closure(closure);
        let runtime_type = generated_runtime_type(module, location);
        let root = roots.for_generated(location);
        let identity = lir::TypeDescriptorIdentity::new(runtime_type, root.clone())
            .expect("a closure exact type derives its descriptor identity");
        let instance_layout = lir::LayoutIdentity::managed_object(
            runtime_type.exact_type(),
            context.target_profile(),
            root,
        )
        .expect("a closure exact type derives its instance layout identity");
        assert_eq!(
            def.bridges.len(),
            1,
            "a closure has one fixed dynamic invoke"
        );
        let vtable = lir::VtableRecord::new(
            &identity,
            vec![lir::DispatchEntry {
                callable: lir::CallableRef::Local(
                    local_functions[&def.bridges[0].function].declaration(),
                ),
            }],
        )
        .expect("a closure exact type derives its dynamic invoke table");
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            relations: Default::default(),
            diagnostic_name: def.name.clone(),
            identity,
            instance_layout,
            instance_shape: lir::TypeInstanceShapeV1::fixed_object(
                context.target_profile(),
                size,
                align,
                scan,
            )?,
            inline_scan: lir::TypeDescriptorInlineScanV1::Null,
            parent: Some(refs.functions[&def.function_type]),
            vtable,
            itables: Vec::new(),
        });
        refs.closures
            .insert(closure, lir::TypeDescriptorRef::Local(descriptor));
    }
    Ok(())
}

fn required_function_types(module: &mir::Module) -> Vec<mir::FunctionTypeId> {
    let mut required = std::collections::HashSet::new();
    for (_, closure) in module.closure_classes.iter() {
        required.insert(closure.function_type);
    }
    for (_, function) in module.functions.iter() {
        for (_, block) in function.body.blocks.iter() {
            mir::visit_block_exprs(block, &mut |expression| {
                if let mir::ExprKind::IsInstance { check_ty, .. } = &expression.kind
                    && let mir::Type::Function(function) = check_ty.as_ref()
                {
                    required.insert(*function);
                }
            });
        }
    }
    let mut required = required.into_iter().collect::<Vec<_>>();
    required.sort_unstable_by_key(|function| {
        exact_type_record(module, &mir::Type::Function(*function)).id()
    });
    let mut ordered = Vec::new();
    let mut visited = std::collections::HashSet::new();
    for function in required {
        order_function_type(module, function, &mut visited, &mut ordered);
    }
    ordered
}

fn order_function_type(
    module: &mir::Module,
    function: mir::FunctionTypeId,
    visited: &mut std::collections::HashSet<mir::FunctionTypeId>,
    ordered: &mut Vec<mir::FunctionTypeId>,
) {
    if !visited.insert(function) {
        return;
    }
    let signature = &module.function_types[function];
    for ty in signature
        .parameter_types
        .iter()
        .chain([&signature.return_type])
    {
        if let mir::Type::Function(nested) = ty {
            order_function_type(module, *nested, visited, ordered);
        }
    }
    ordered.push(function);
}

fn function_operand(refs: &TypeDescriptorRefs, ty: &mir::Type) -> Option<lir::TypeDescriptorRef> {
    (!matches!(ty, mir::Type::Any)).then(|| refs.for_type(ty))
}
