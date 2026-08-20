use super::*;

#[derive(Debug)]
pub struct StructDef {
    /// Exact semantic identity, independent of this store's local index.
    pub exact_type: scoop_identity::PersistentExactTypeId,
    pub name: String,
    pub size: u64,
    pub align: u64,
    pub interior_mutable: bool,
    pub representation: StructRepresentation,
}

/// Immutable-by-classification struct definition store. C-layout references
/// can only be minted here, and the only post-allocation updates preserve the
/// representation family that justified the refinement.
#[derive(Debug, Default)]
pub struct StructDefs {
    definitions: Arena<StructDef>,
}

/// A Scoop-layout definition allocated by the classification-preserving store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScoopStructRef(StructDefId);

impl ScoopStructRef {
    pub const fn definition(self) -> StructDefId {
        self.0
    }
}

/// Borrowed C payload selected together with its common nominal metadata.
#[derive(Debug, Clone, Copy)]
pub struct CStructView<'a> {
    pub definition: &'a StructDef,
    pub contract: LirCLayoutContract,
    pub fields: &'a [CStructField],
}

impl StructDefs {
    pub fn alloc_scoop(
        &mut self,
        exact_type: scoop_identity::PersistentExactTypeId,
        name: String,
        size: u64,
        align: u64,
        interior_mutable: bool,
        fields: Vec<StructField>,
    ) -> ScoopStructRef {
        ScoopStructRef(self.definitions.alloc(StructDef {
            exact_type,
            name,
            size,
            align,
            interior_mutable,
            representation: StructRepresentation::Scoop { fields },
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn alloc_c(
        &mut self,
        exact_type: scoop_identity::PersistentExactTypeId,
        name: String,
        size: u64,
        align: u64,
        interior_mutable: bool,
        contract: LirCLayoutContract,
        fields: Vec<CStructField>,
    ) -> CStructRef {
        let id = self.definitions.alloc(StructDef {
            exact_type,
            name,
            size,
            align,
            interior_mutable,
            representation: StructRepresentation::C { contract, fields },
        });
        CStructRef::from_validated_definition(id)
    }

    pub fn alloc_intrinsic(
        &mut self,
        exact_type: scoop_identity::PersistentExactTypeId,
        name: String,
        size: u64,
        align: u64,
        representation: IntrinsicTypeRepresentation,
    ) -> StructDefId {
        self.definitions.alloc(StructDef {
            exact_type,
            name,
            size,
            align,
            interior_mutable: false,
            representation: StructRepresentation::Intrinsic(representation),
        })
    }

    pub fn set_scoop_fields(&mut self, reference: ScoopStructRef, fields: Vec<StructField>) {
        let StructRepresentation::Scoop {
            fields: stored_fields,
        } = &mut self.definitions[reference.definition()].representation
        else {
            unreachable!("ScoopStructRef preserves its store classification")
        };
        *stored_fields = fields;
    }

    pub fn set_c_fields(&mut self, reference: CStructRef, fields: Vec<CStructField>) {
        let StructRepresentation::C {
            fields: stored_fields,
            ..
        } = &mut self.definitions[reference.definition()].representation
        else {
            unreachable!("CStructRef can only identify a C struct")
        };
        *stored_fields = fields;
    }

    pub fn c_ref(&self, id: StructDefId) -> Option<CStructRef> {
        self.definitions[id]
            .is_c_layout()
            .then_some(CStructRef::from_validated_definition(id))
    }

    pub fn c(&self, reference: CStructRef) -> CStructView<'_> {
        let definition = &self.definitions[reference.definition()];
        let StructRepresentation::C { contract, fields } = &definition.representation else {
            unreachable!("CStructRef preserves its store classification")
        };
        CStructView {
            definition,
            contract: *contract,
            fields,
        }
    }

    pub fn iter_c(&self) -> impl Iterator<Item = (CStructRef, CStructView<'_>)> {
        self.definitions.iter().filter_map(|(id, definition)| {
            let StructRepresentation::C { contract, fields } = &definition.representation else {
                return None;
            };
            Some((
                CStructRef::from_validated_definition(id),
                CStructView {
                    definition,
                    contract: *contract,
                    fields,
                },
            ))
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (StructDefId, &StructDef)> {
        self.definitions.iter()
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

impl std::ops::Index<StructDefId> for StructDefs {
    type Output = StructDef;

    fn index(&self, index: StructDefId) -> &Self::Output {
        &self.definitions[index]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub ty: LirType,
    pub layout: FieldLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CStructField {
    pub identity: scoop_identity::PersistentFieldId,
    pub ty: CType,
    pub layout: FieldLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructRepresentation {
    Scoop {
        fields: Vec<StructField>,
    },
    C {
        contract: LirCLayoutContract,
        fields: Vec<CStructField>,
    },
    /// Nominal declaration shell for a compiler pointer intrinsic. Canonical
    /// values use `LirType::Ptr`; this branch prevents the declaration from
    /// being mistaken for an ordinary zero-field aggregate.
    Intrinsic(IntrinsicTypeRepresentation),
}

impl StructDef {
    pub const fn c_layout(&self) -> Option<LirCLayoutContract> {
        match &self.representation {
            StructRepresentation::Scoop { .. } => None,
            StructRepresentation::C { contract, .. } => Some(*contract),
            StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub const fn is_c_layout(&self) -> bool {
        matches!(&self.representation, StructRepresentation::C { .. })
    }

    pub fn scoop_fields(&self) -> Option<&[StructField]> {
        match &self.representation {
            StructRepresentation::Scoop { fields } => Some(fields),
            StructRepresentation::C { .. } | StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub fn c_fields(&self) -> Option<&[CStructField]> {
        match &self.representation {
            StructRepresentation::Scoop { .. } => None,
            StructRepresentation::C { fields, .. } => Some(fields),
            StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub fn field_count(&self) -> usize {
        match &self.representation {
            StructRepresentation::Scoop { fields } => fields.len(),
            StructRepresentation::C { fields, .. } => fields.len(),
            StructRepresentation::Intrinsic(_) => 0,
        }
    }

    pub fn field_layout(&self, index: usize) -> Option<FieldLayout> {
        match &self.representation {
            StructRepresentation::Scoop { fields } => fields.get(index).map(|field| field.layout),
            StructRepresentation::C { fields, .. } => fields.get(index).map(|field| field.layout),
            StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub fn field_storage_type(&self, index: usize) -> Option<LirType> {
        match &self.representation {
            StructRepresentation::Scoop { fields } => {
                fields.get(index).map(|field| field.ty.clone())
            }
            StructRepresentation::C { fields, .. } => {
                fields.get(index).map(|field| field.ty.storage_type())
            }
            StructRepresentation::Intrinsic(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldLayout {
    pub offset: u64,
    /// Alignment that a load/store of this field may claim. Packed layouts
    /// cap this independently of the field type's natural alignment.
    pub access_align: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LirCLayoutValue {
    Natural,
    A1,
    A2,
    A4,
    A8,
    A16,
}

impl LirCLayoutValue {
    pub const fn bytes(self) -> Option<u64> {
        match self {
            Self::Natural => None,
            Self::A1 => Some(1),
            Self::A2 => Some(2),
            Self::A4 => Some(4),
            Self::A8 => Some(8),
            Self::A16 => Some(16),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LirCLayoutContract {
    pub aligned: LirCLayoutValue,
    pub packed: LirCLayoutValue,
}
