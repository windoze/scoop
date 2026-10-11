//! Local references to selected enum variants and declared struct fields.

use super::*;

/// Checked local-concrete identity of one enum variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantRef {
    enumeration: EnumId,
    variant: VariantId,
}

impl EnumVariantRef {
    pub fn checked(
        enums: &Arena<EnumDef>,
        enumeration: EnumId,
        variant: VariantId,
    ) -> Option<Self> {
        if enumeration.into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        enums[enumeration]
            .variants
            .get(variant.into_raw() as usize)
            .map(|_| Self {
                enumeration,
                variant,
            })
    }

    pub const fn enumeration(self) -> EnumId {
        self.enumeration
    }

    pub const fn variant(self) -> VariantId {
        self.variant
    }
}

/// Checked local-concrete identity of one payload field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantFieldRef {
    variant: EnumVariantRef,
    local_index: u32,
}

impl EnumVariantFieldRef {
    pub fn checked(
        enums: &Arena<EnumDef>,
        variant: EnumVariantRef,
        local_index: u32,
    ) -> Option<Self> {
        if variant.enumeration().into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        let declaration = enums[variant.enumeration()]
            .variants
            .get(variant.variant().into_raw() as usize)?;
        declaration.fields.get(local_index as usize).map(|_| Self {
            variant,
            local_index,
        })
    }

    pub const fn variant(self) -> EnumVariantRef {
        self.variant
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
}

/// Complete local-concrete identity of one specialized core `Option` shape.
/// The Some payload and None variants are inseparable from their checked
/// owner, and their arities are verified when this value is constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptionCore {
    some_payload: EnumVariantFieldRef,
    none: EnumVariantRef,
}

impl OptionCore {
    pub fn checked(
        enums: &Arena<EnumDef>,
        some_payload: EnumVariantFieldRef,
        none: EnumVariantRef,
    ) -> Option<Self> {
        let some = some_payload.variant();
        if some.enumeration() != none.enumeration() || some.variant() == none.variant() {
            return None;
        }
        if some.enumeration().into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        let enumeration = &enums[some.enumeration()];
        let [argument] = enumeration.type_arguments.as_slice() else {
            return None;
        };
        let some_definition = enumeration
            .variants
            .get(some.variant().into_raw() as usize)?;
        let none_definition = enumeration
            .variants
            .get(none.variant().into_raw() as usize)?;
        (enumeration.variants.len() == 2
            && some_definition.name == "Some"
            && some_definition.fields.len() == 1
            && some_payload.local_index() == 0
            && some_definition
                .fields
                .get(some_payload.local_index() as usize)
                .is_some_and(|field| field.ty == *argument)
            && none_definition.name == "None"
            && none_definition.fields.is_empty())
        .then_some(Self { some_payload, none })
    }

    pub const fn enumeration(self) -> EnumId {
        self.some_payload.variant().enumeration()
    }

    pub const fn some(self) -> EnumVariantRef {
        self.some_payload.variant()
    }

    pub const fn some_payload(self) -> EnumVariantFieldRef {
        self.some_payload
    }

    pub const fn none(self) -> EnumVariantRef {
        self.none
    }
}

/// Checked local-concrete identity of one declared struct field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructFieldRef {
    structure: StructId,
    local_index: u32,
}

impl StructFieldRef {
    pub fn resolve(
        structs: &Arena<StructDef>,
        structure: StructId,
        field: scoop_identity::PersistentFieldId,
    ) -> Option<Self> {
        if structure.into_raw().into_u32() as usize >= structs.len() {
            return None;
        }
        let StructRepresentation::Declared(DeclaredStruct { fields, .. }) =
            &structs[structure].representation
        else {
            return None;
        };
        let local_index = fields
            .iter()
            .position(|candidate| candidate.identity == field)?;
        Some(Self {
            structure,
            local_index: u32::try_from(local_index).ok()?,
        })
    }

    pub const fn structure(self) -> StructId {
        self.structure
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
}
