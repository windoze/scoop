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
        if let Some(&existing) = self.class_constructor_by_key.get(&(source, owner)) {
            return existing;
        }
        let raw = u32::try_from(self.class_constructor_slots.len())
            .expect("concrete constructor ids fit in u32");
        let id = concrete::ClassConstructorId::from_raw(raw.into());
        self.class_constructor_slots.push(None);
        self.class_constructor_keys.push((source, owner));
        self.class_constructor_by_key.insert((source, owner), id);
        self.pending_constructors
            .push_back(ConstructorWork::Class(id));
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
        if let Some(&existing) = self.struct_constructor_by_key.get(&(source, owner)) {
            return existing;
        }
        let raw = u32::try_from(self.struct_constructor_slots.len())
            .expect("concrete constructor ids fit in u32");
        let id = concrete::StructConstructorId::from_raw(raw.into());
        self.struct_constructor_slots.push(None);
        self.struct_constructor_keys.push((source, owner));
        self.struct_constructor_by_key.insert((source, owner), id);
        self.pending_constructors
            .push_back(ConstructorWork::Struct(id));
        id
    }

    pub(super) fn lower_pending_constructor(&mut self, work: ConstructorWork) {
        match work {
            ConstructorWork::Class(id) => {
                let index = id.into_raw().into_u32() as usize;
                let (source, owner) = self.class_constructor_keys[index];
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
                let (source, owner) = self.struct_constructor_keys[index];
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
