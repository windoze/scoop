use super::*;

impl Lowerer {
    pub(super) fn lower_singletons(&mut self, module: &hir::Module) {
        for (source_id, source) in module.object_types.iter() {
            let representation = self.class_map[&source.representation];
            let id = self.object_types.alloc(mir::ObjectType {
                declaration: mir::ObjectId::from_raw(source.declaration.into_raw()),
                representation,
            });
            assert_eq!(source_id.into_raw(), id.into_raw());
        }

        for (source_id, source) in module.objects.iter() {
            let id = self.objects.alloc(mir::ObjectDef {
                name: source.name.clone(),
                object_type: mir::ObjectTypeId::from_raw(source.object_type.into_raw()),
                singleton_value: mir::SingletonValueId::from_raw(source.singleton_value.into_raw()),
                backing_class: self.class_map[&source.backing_class],
            });
            assert_eq!(source_id.into_raw(), id.into_raw());
        }

        for (source_id, source) in module.singleton_published_roots.iter() {
            let value = &module.singleton_values[source.value];
            let owner = module.objects[value.declaration]
                .origin
                .concrete_type_id()
                .expect("a materialized singleton has a concrete nominal identity");
            let ty = {
                let types = Types {
                    module,
                    struct_map: &self.struct_map,
                    class_map: &self.class_map,
                };
                types.lower(
                    source.ty,
                    &mut self.source_exact_types,
                    &mut self.enums,
                    &mut self.structs,
                    &mut self.interfaces,
                    &mut self.shell,
                )
            };
            let global = self.globals.alloc(mir::Global {
                name: format!("$singleton${}", source.link_name),
                storage_owner: mir::StaticStorageOwner::SingletonPublishedRoot(owner),
                ty,
                mutable: true,
                storage: mir::GlobalStorage::Managed {
                    initial_state: mir::MirStaticInitialState::ZeroedForRuntimeUnit,
                },
            });
            let id = self
                .singleton_published_roots
                .alloc(mir::SingletonPublishedRoot {
                    value: mir::SingletonValueId::from_raw(source.value.into_raw()),
                    global,
                });
            assert_eq!(source_id.into_raw(), id.into_raw());
            self.singleton_root_map.insert(source_id, id);
        }

        for (source_id, source) in module.singleton_values.iter() {
            let id = self.singleton_values.alloc(mir::SingletonValue {
                declaration: mir::ObjectId::from_raw(source.declaration.into_raw()),
                object_type: mir::ObjectTypeId::from_raw(source.object_type.into_raw()),
                published_root: self.singleton_root_map[&source.published_root],
                initialization: mir::InitializationUnitId::from_raw(
                    source.initialization.into_raw(),
                ),
            });
            assert_eq!(source_id.into_raw(), id.into_raw());
        }
    }
}
