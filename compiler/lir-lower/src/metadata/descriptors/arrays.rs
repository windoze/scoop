use super::*;

/// Transpose every concrete intrinsic array class application into one typed
/// LIR metadata record. The MIR class id -> LIR array id map is complete before
/// function lowering starts, so no instruction discovers metadata on demand.
pub(crate) fn array_types(
    context: &LoweringContext,
    identity_roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    descriptors: &TypeDescriptorRefs,
) -> StorageResult<(
    Arena<lir::ArrayType>,
    HashMap<mir::ClassId, lir::ArrayTypeId>,
)> {
    let mut arrays = Arena::new();
    let mut ids = HashMap::new();
    for (class_id, class) in module.classes.iter() {
        let ty = mir::Type::Class(class_id);
        if !identity_roots.materializes_type(&ty) {
            continue;
        }
        let (kind, element) = match &class.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Array {
                element,
            }) => (lir::ArrayKind::Immutable, element),
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::MutableArray { element },
            ) => (lir::ArrayKind::Mutable, element),
            mir::ClassRepresentation::Declared { .. } | mir::ClassRepresentation::Intrinsic(_) => {
                continue;
            }
        };
        let (element_size, element_align, element_scan) =
            class_layout(context, module, enums, class)?;
        let id = arrays.alloc(lir::ArrayType {
            identity: lir::LayoutIdentity::managed_array(
                exact_type_record(module, &ty).id(),
                context.target_profile(),
                identity_roots.for_type(&ty),
            )
            .expect("validated array exact type and target must derive layout identities"),
            kind,
            element_exact: exact_type_record(module, element).id(),
            element: lir_type(element),
            layout: lir::ArrayLayoutV1::new(
                context.target_profile(),
                lir::ArrayElementStorageV1::from_value(&value_storage(
                    context,
                    element_size,
                    element_align,
                    element_scan,
                )?),
            )?,
            type_descriptor: descriptors.classes[&class_id],
        });
        assert!(ids.insert(class_id, id).is_none());
    }
    Ok((arrays, ids))
}
