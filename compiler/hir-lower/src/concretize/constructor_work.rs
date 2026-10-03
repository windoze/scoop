//! On-demand constructor bodies participate in the callable fixed point.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum ConstructorWork {
    Class(concrete::ClassConstructorId),
    Struct(concrete::StructConstructorId),
}

impl Concretizer<'_> {
    pub(super) fn request_class_constructor(
        &mut self,
        source: export::ClassConstructorId,
        owner: concrete::ClassId,
    ) -> concrete::ClassConstructorId {
        assert_eq!(
            self.source.class_constructors[source].owner,
            self.class_source[&owner]
        );
        self.request_class_constructor_source(
            export::ClassConstructorDefinition::Local(source),
            owner,
        )
    }

    pub(super) fn request_class_constructor_source(
        &mut self,
        source: export::ClassConstructorDefinition,
        owner: concrete::ClassId,
    ) -> concrete::ClassConstructorId {
        let key = (self.class_constructor_origin(source).0, owner);
        if let Some(&existing) = self.class_constructor_by_key.get(&key) {
            return existing;
        }
        let raw = u32::try_from(self.class_constructor_slots.len())
            .expect("concrete constructor ids fit in u32");
        let id = concrete::ClassConstructorId::from_raw(raw.into());
        self.class_constructor_slots.push(None);
        self.class_constructor_definitions.push((source, owner));
        self.class_constructor_by_key.insert(key, id);
        self.pending_constructors
            .push_back((ConstructorWork::Class(id), self.type_use_site));
        id
    }

    pub(super) fn request_struct_constructor(
        &mut self,
        source: export::StructConstructorId,
        owner: concrete::StructId,
    ) -> concrete::StructConstructorId {
        assert_eq!(
            self.source.struct_constructors[source].owner,
            self.struct_source[&owner]
        );
        self.request_struct_constructor_source(
            export::StructConstructorDefinition::Local(source),
            owner,
        )
    }

    pub(super) fn request_struct_constructor_source(
        &mut self,
        source: export::StructConstructorDefinition,
        owner: concrete::StructId,
    ) -> concrete::StructConstructorId {
        let key = (self.struct_constructor_origin(source), owner);
        if let Some(&existing) = self.struct_constructor_by_key.get(&key) {
            return existing;
        }
        let raw = u32::try_from(self.struct_constructor_slots.len())
            .expect("concrete constructor ids fit in u32");
        let id = concrete::StructConstructorId::from_raw(raw.into());
        self.struct_constructor_slots.push(None);
        self.struct_constructor_definitions.push((source, owner));
        self.struct_constructor_by_key.insert(key, id);
        self.pending_constructors
            .push_back((ConstructorWork::Struct(id), self.type_use_site));
        id
    }

    pub(super) fn lower_pending_constructor(&mut self, work: ConstructorWork) {
        match work {
            ConstructorWork::Class(id) => {
                let index = id.into_raw().into_u32() as usize;
                let (source, owner) = self.class_constructor_definitions[index];
                let arguments = self.classes[owner].type_arguments.clone();
                let constructor = self.lower_class_constructor(source, owner, &arguments);
                assert!(
                    self.class_constructor_slots[index]
                        .replace(constructor)
                        .is_none()
                );
            }
            ConstructorWork::Struct(id) => {
                let index = id.into_raw().into_u32() as usize;
                let (source, owner) = self.struct_constructor_definitions[index];
                let arguments = self.structs[owner].type_arguments.clone();
                let constructor = self.lower_struct_constructor(source, owner, &arguments);
                assert!(
                    self.struct_constructor_slots[index]
                        .replace(constructor)
                        .is_none()
                );
            }
        }
    }
}
