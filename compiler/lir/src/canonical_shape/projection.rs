use crate::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) enum ShapeContent<'a> {
    Fixed(&'a Layout),
    Array(&'a ArrayType),
    Instance(&'a TypeDescriptor),
    Scan(&'a RefScan),
    Descriptor(&'a TypeDescriptor),
    Dispatch(PersistentDispatchTableId, &'a [DispatchEntry]),
    Immortal(StrongImmortalObjectSemanticPlanV1, &'a str),
}

pub(super) struct ShapeContents<'a> {
    values: BTreeMap<StrongDefinitionEntity, ShapeContent<'a>>,
}

impl<'a> ShapeContents<'a> {
    pub(super) fn new(
        module: &'a Module,
        immortals: impl IntoIterator<Item = StrongImmortalObjectSemanticPlanV1>,
    ) -> Self {
        let mut values = BTreeMap::new();
        let immortals = immortals
            .into_iter()
            .map(|plan| (plan.object(), plan))
            .collect::<BTreeMap<_, _>>();
        for (_, layout) in module.meta.layouts.iter() {
            values.insert(
                StrongDefinitionEntity::layout(layout.identity.layout_record().id()),
                ShapeContent::Fixed(layout),
            );
            let scan = match &layout.kind {
                LayoutKind::Plain { scan } | LayoutKind::Enum { scan } => scan,
                LayoutKind::Intrinsic(_) => &RefScan::None,
            };
            values.insert(
                StrongDefinitionEntity::scan(layout.identity.scan_record().id()),
                ShapeContent::Scan(scan),
            );
        }
        for (_, array) in module.meta.arrays.iter() {
            values.insert(
                StrongDefinitionEntity::layout(array.identity.layout_record().id()),
                ShapeContent::Array(array),
            );
            values.insert(
                StrongDefinitionEntity::scan(array.identity.scan_record().id()),
                ShapeContent::Scan(array.layout.instance().inline_scan()),
            );
        }
        for (_, descriptor) in module.meta.type_descriptors.iter() {
            values
                .entry(StrongDefinitionEntity::layout(
                    descriptor.instance_layout.layout_record().id(),
                ))
                .or_insert(ShapeContent::Instance(descriptor));
            values.insert(
                StrongDefinitionEntity::scan(descriptor.instance_layout.scan_record().id()),
                ShapeContent::Scan(descriptor.instance_shape.object_scan()),
            );
            values.insert(
                StrongDefinitionEntity::exact_type(descriptor.identity.exact_type()),
                ShapeContent::Descriptor(descriptor),
            );
            let table = descriptor.vtable.identity_record().id();
            values.insert(
                StrongDefinitionEntity::dispatch_table(table),
                ShapeContent::Dispatch(table, descriptor.vtable.slots()),
            );
            for table in &descriptor.itables {
                values.insert(
                    StrongDefinitionEntity::dispatch_table(table.identity_record().id()),
                    ShapeContent::Dispatch(table.identity_record().id(), table.slots()),
                );
            }
        }
        for (_, global) in module.globals.iter() {
            if let GlobalInit::StringConst { identity, value } = &global.init {
                let object = identity.identity_record().id();
                values.insert(
                    StrongDefinitionEntity::immortal_object(object),
                    ShapeContent::Immortal(immortals[&object], value),
                );
            }
            if let GlobalInit::Storage {
                layout: StaticStorageLayout::Local(layout),
                ..
            } = &global.init
            {
                values.insert(
                    StrongDefinitionEntity::scan(layout.scan_record().id()),
                    ShapeContent::Scan(&global.scan),
                );
            }
        }
        Self { values }
    }

    pub(super) fn get(&self, entity: StrongDefinitionEntity) -> Option<ShapeContent<'a>> {
        self.values.get(&entity).copied()
    }
}
