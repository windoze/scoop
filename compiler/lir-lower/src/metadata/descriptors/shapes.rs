use super::*;

pub(super) fn value_or_abstract_type_descriptor(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    ty: &mir::Type,
    exact: scoop_identity::PersistentExactTypeId,
    root: lir::MaterializationRoot,
) -> StorageResult<lir::TypeDescriptor> {
    let identity = lir::TypeDescriptorIdentity::new(
        lir::RuntimeTypeMappingRecord::new(exact)
            .expect("validated exact type must derive a nonzero runtime id"),
        root.clone(),
    )
    .expect("validated exact type must derive descriptor identities");
    let instance_layout =
        lir::LayoutIdentity::managed_object(exact, context.target_profile(), root.clone())
            .expect("validated exact type and target must derive its instance layout identity");
    let (instance_shape, inline_scan) = if matches!(ty, mir::Type::Any) {
        (
            lir::TypeInstanceShapeV1::abstract_ref(),
            lir::TypeDescriptorInlineScanV1::Null,
        )
    } else {
        assert!(
            !matches!(
                ty,
                mir::Type::String
                    | mir::Type::Class(_)
                    | mir::Type::Interface(_)
                    | mir::Type::Function(_)
            ),
            "reference source nominals have dedicated descriptors"
        );
        let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
        let (size, align) = size_align(context, module, &enum_shape, ty)?;
        let scan = ref_scan(context, module, enums, ty, 0)?;
        let inline_scan = typed_inline_scan(
            &scan,
            lir::LayoutIdentity::managed_value(
                exact_type_record(module, ty).id(),
                context.target_profile(),
                root,
            )
            .expect("validated value type must derive its inline scan identity")
            .scan_record()
            .id(),
        );
        let value = value_storage(context, size, align, scan)?;
        (
            lir::TypeInstanceShapeV1::boxed_value(context.target_profile(), value)?,
            inline_scan,
        )
    };
    let vtable = lir::VtableRecord::new(&identity, Vec::new())
        .expect("validated exact type must derive a vtable identity");
    Ok(lir::TypeDescriptor {
        diagnostic_name: mir::type_name(module, ty),
        identity,
        instance_layout,
        instance_shape,
        inline_scan,
        parent: None,
        vtable,
        itables: Vec::new(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn class_type_descriptor(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    id: mir::ClassId,
    refs: &TypeDescriptorRefs,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
    external_callables: &HashMap<mir::ExternalCallableUseId, lir::ExternalCallableId>,
) -> StorageResult<lir::TypeDescriptor> {
    let def = &module.classes[id];
    let descriptor_type = if matches!(
        def.representation,
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
    ) {
        mir::Type::String
    } else {
        mir::Type::Class(id)
    };
    let runtime_type = runtime_type(module, &descriptor_type);
    let root = identity_roots.for_type(&descriptor_type);
    let identity = lir::TypeDescriptorIdentity::new(runtime_type, root.clone())
        .expect("validated class exact type must derive descriptor identities");
    let instance_layout = lir::LayoutIdentity::managed_object(
        runtime_type.exact_type(),
        context.target_profile(),
        root.clone(),
    )
    .expect("validated class exact type must derive its instance layout identity");
    let (instance_shape, inline_scan) = match &def.representation {
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => (
            lir::TypeInstanceShapeV1::inline_bytes(context.target_profile())?,
            lir::TypeDescriptorInlineScanV1::Null,
        ),
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { .. }
            | mir::IntrinsicTypeRepresentation::MutableArray { .. },
        ) => {
            let (size, align, scan) = class_layout(context, module, enums, def)?;
            let inline_scan = typed_inline_scan(
                &scan,
                lir::LayoutIdentity::managed_array(
                    runtime_type.exact_type(),
                    context.target_profile(),
                    root.clone(),
                )
                .expect("validated array must derive its element scan identity")
                .scan_record()
                .id(),
            );
            let element =
                lir::ArrayElementStorageV1::from_value(&value_storage(context, size, align, scan)?);
            (
                lir::TypeInstanceShapeV1::inline_array(context.target_profile(), element)?,
                inline_scan,
            )
        }
        mir::ClassRepresentation::Declared { .. } => {
            if let Some(boxed) = module
                .meta
                .boxed_types
                .iter()
                .find(|boxed| boxed.class() == id)
            {
                let enum_shape =
                    |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
                let (size, align) = size_align(context, module, &enum_shape, boxed.payload())?;
                let scan = ref_scan(context, module, enums, boxed.payload(), 0)?;
                let inline_scan = typed_inline_scan(
                    &scan,
                    lir::LayoutIdentity::managed_value(
                        exact_type_record(module, boxed.payload()).id(),
                        context.target_profile(),
                        identity_roots.for_type(boxed.payload()),
                    )
                    .expect("validated box payload must derive its inline scan identity")
                    .scan_record()
                    .id(),
                );
                let value = value_storage(context, size, align, scan)?;
                (
                    lir::TypeInstanceShapeV1::boxed_value(context.target_profile(), value)?,
                    inline_scan,
                )
            } else {
                let (size, align, scan) = class_layout(context, module, enums, def)?;
                (
                    lir::TypeInstanceShapeV1::fixed_object(
                        context.target_profile(),
                        size,
                        align,
                        scan,
                    )?,
                    lir::TypeDescriptorInlineScanV1::Null,
                )
            }
        }
        mir::ClassRepresentation::Intrinsic(_) => {
            unreachable!("the registry fixes intrinsic declaration targets")
        }
    };
    let vtable = lir::VtableRecord::new(
        &identity,
        def.vtable
            .iter()
            .map(|slot| dispatch_entry(slot, local_functions, external_callables))
            .collect(),
    )
    .expect("validated class exact type must derive a vtable identity");
    let mut itables: Vec<_> = def
        .itables
        .iter()
        .filter_map(|record| {
            let interface = refs.interfaces.get(&record.interface).copied()?;
            lir::ItableRecord::new(
                &identity,
                exact_type_record(module, &mir::Type::Interface(record.interface)).id(),
                interface,
                record
                    .slots
                    .iter()
                    .map(|slot| dispatch_entry(slot, local_functions, external_callables))
                    .collect(),
            )
            .expect("validated class and interface exact types must derive an itable identity")
            .into()
        })
        .collect();
    itables.sort_unstable_by_key(|table| table.identity_record().key().interface());
    Ok(lir::TypeDescriptor {
        diagnostic_name: def.name.clone(),
        identity,
        instance_layout,
        instance_shape,
        inline_scan,
        parent: def
            .base_class()
            .and_then(|base| refs.classes.get(&base).copied()),
        vtable,
        itables,
    })
}

fn typed_inline_scan(
    scan: &lir::RefScan,
    definition: scoop_identity::PersistentScanId,
) -> lir::TypeDescriptorInlineScanV1 {
    if scan.contains_reference() {
        lir::TypeDescriptorInlineScanV1::Defined(definition)
    } else {
        lir::TypeDescriptorInlineScanV1::Null
    }
}

pub(super) fn runtime_type(module: &mir::Module, ty: &mir::Type) -> lir::RuntimeTypeMappingRecord {
    lir::RuntimeTypeMappingRecord::new(exact_type_record(module, ty).id())
        .expect("validated exact type must derive a nonzero runtime id")
}

pub(super) fn generated_runtime_type(
    module: &mir::Module,
    location: mir::GeneratedExactTypeLocation,
) -> lir::RuntimeTypeMappingRecord {
    lir::RuntimeTypeMappingRecord::new(generated_exact_type_record(module, location).id())
        .expect("validated generated exact type must derive a nonzero runtime id")
}
