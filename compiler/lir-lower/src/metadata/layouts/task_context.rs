use super::*;

pub(super) fn append(
    context: &LoweringContext,
    roots: &IdentityRoots<'_>,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    layouts: &mut Arena<lir::Layout>,
) -> StorageResult<()> {
    for root in roots.generated_nominal_shapes() {
        let mir::GeneratedExactTypeLocation::Context(storage) = root.location() else {
            continue;
        };
        let ty = mir::Type::Context(storage);
        let identity = managed_value_layout_identity(context, roots, module, &ty);
        if storage.role.is_reference() {
            layouts.alloc(managed_reference_value_layout(
                context,
                identity,
                storage.role.name(),
            ));
        } else {
            let pointer = context.pointer_layout(lir::PointerKind::Managed);
            let count = mir::context_fields(storage).len() as u64;
            let scan = ref_scan(context, module, enums, &ty, 0)?;
            layouts.alloc(lir::Layout {
                identity,
                name: storage.role.name().to_string(),
                size: count * pointer.size,
                align: pointer.align,
                fields: (0..count)
                    .map(|index| lir::FieldLayout {
                        offset: index * pointer.size,
                        access_align: pointer.align,
                    })
                    .collect(),
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Plain { scan },
            });
        }
        if matches!(
            storage.role,
            mir::ContextStorageRole::Task | mir::ContextStorageRole::Node
        ) {
            let pointer = context.pointer_layout(lir::PointerKind::Managed);
            let header = context.object_header_layout();
            let count = mir::context_fields(storage).len() as u64;
            let fields: Vec<_> = (0..count)
                .map(|index| lir::FieldLayout {
                    offset: header.size + index * pointer.size,
                    access_align: pointer.align,
                })
                .collect();
            let scan = lir::RefScan::References(fields.iter().map(|field| field.offset).collect());
            layouts.alloc(lir::Layout {
                identity: managed_object_layout_identity(context, roots, module, &ty),
                name: storage.role.name().to_string(),
                size: header.size + count * pointer.size,
                align: header.align.max(pointer.align),
                fields,
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Plain { scan },
            });
        }
    }
    Ok(())
}
