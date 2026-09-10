use la_arena::Arena;
use scoop_hir::concrete;

#[derive(Debug, Clone)]
pub(super) struct PendingClassConstructor {
    pub(super) class: concrete::ClassId,
    pub(super) source_discriminator: u32,
    pub(super) parameters: Vec<concrete::ConstructorParameter>,
    pub(super) kind: concrete::ClassConstructorKind,
}

#[derive(Debug, Clone)]
pub(super) struct PendingStructConstructor {
    pub(super) structure: concrete::StructId,
    pub(super) source_discriminator: u32,
    pub(super) parameters: Vec<concrete::ConstructorParameter>,
    pub(super) kind: concrete::StructConstructorKind,
}

pub(super) fn finish_class_constructor_slots(
    slots: Vec<Option<PendingClassConstructor>>,
    materializations: Vec<concrete::CallableMaterialization>,
) -> Arena<concrete::ClassConstructor> {
    assert_eq!(slots.len(), materializations.len());
    let mut arena = Arena::new();
    for (index, (slot, materialization)) in slots.into_iter().zip(materializations).enumerate() {
        let pending =
            slot.unwrap_or_else(|| panic!("missing concrete class constructor at index {index}"));
        let id = arena.alloc(concrete::ClassConstructor {
            class: pending.class,
            materialization,
            source_discriminator: pending.source_discriminator,
            parameters: pending.parameters,
            kind: pending.kind,
        });
        assert_eq!(id.into_raw().into_u32() as usize, index);
    }
    arena
}

pub(super) fn finish_struct_constructor_slots(
    slots: Vec<Option<PendingStructConstructor>>,
    materializations: Vec<concrete::CallableMaterialization>,
) -> Arena<concrete::StructConstructor> {
    assert_eq!(slots.len(), materializations.len());
    let mut arena = Arena::new();
    for (index, (slot, materialization)) in slots.into_iter().zip(materializations).enumerate() {
        let pending =
            slot.unwrap_or_else(|| panic!("missing concrete struct constructor at index {index}"));
        let id = arena.alloc(concrete::StructConstructor {
            structure: pending.structure,
            materialization,
            source_discriminator: pending.source_discriminator,
            parameters: pending.parameters,
            kind: pending.kind,
        });
        assert_eq!(id.into_raw().into_u32() as usize, index);
    }
    arena
}
