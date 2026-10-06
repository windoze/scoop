use super::*;

/// Declaration-side identity of one source-visible field of one struct.
/// Intrinsic struct representations cannot construct this ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructFieldRef {
    structure: StructId,
    local_index: u32,
}

impl StructFieldRef {
    pub fn checked(
        structs: &Arena<StructDecl>,
        structure: StructId,
        local_index: u32,
    ) -> Option<Self> {
        if structure.into_raw().into_u32() as usize >= structs.len() {
            return None;
        }
        structs[structure]
            .semantic_fields()
            .get(local_index as usize)
            .map(|_| Self {
                structure,
                local_index,
            })
    }

    pub const fn structure(self) -> StructId {
        self.structure
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
}

/// Exact export-side identity of one source-visible field of one struct
/// application. It retains the checked declaration relation instead of an
/// untyped ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppliedStructFieldRef {
    application: StructApplicationId,
    declaration: StructFieldRef,
}

impl AppliedStructFieldRef {
    pub fn checked(
        structs: &Arena<StructDecl>,
        applications: &Arena<StructApplication>,
        nominal_identities: &HirNominalIdentities,
        application: StructApplicationId,
        local_index: u32,
    ) -> Option<Self> {
        if application.into_raw().into_u32() as usize >= applications.len() {
            return None;
        }
        let structure = nominal_identities.struct_id(applications[application].template)?;
        let declaration = StructFieldRef::checked(structs, structure, local_index)?;
        Some(Self {
            application,
            declaration,
        })
    }

    pub const fn application(self) -> StructApplicationId {
        self.application
    }

    pub const fn declaration(self) -> StructFieldRef {
        self.declaration
    }

    pub const fn local_index(self) -> u32 {
        self.declaration.local_index()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalFieldStorage {
    Declared,
    PropertyBacking(scoop_identity::PersistentPropertyId),
    PropertyDelegate(scoop_identity::PersistentPropertyId),
    Generated,
}

impl NominalFieldStorage {
    pub const fn backing_property(self) -> Option<scoop_identity::PersistentPropertyId> {
        match self {
            Self::PropertyBacking(property) => Some(property),
            Self::Declared | Self::PropertyDelegate(_) | Self::Generated => None,
        }
    }
}

impl NominalFieldStorage {
    pub fn from_key(key: &scoop_identity::FieldIdentityKey) -> Self {
        use scoop_identity::FieldIdentityView;
        match key.view() {
            FieldIdentityView::SourceDeclared { .. } => Self::Declared,
            FieldIdentityView::SourcePropertyBacking { property, .. } => {
                Self::PropertyBacking(property)
            }
            FieldIdentityView::SourcePropertyDelegate { property, .. } => {
                Self::PropertyDelegate(property)
            }
            FieldIdentityView::Generated { key, .. } => key
                .object_backing_property()
                .map_or(Self::Generated, Self::PropertyBacking),
        }
    }
}
