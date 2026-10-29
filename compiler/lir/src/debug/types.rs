use scoop_identity::PersistentExactTypeId;
use std::collections::BTreeMap;

/// Session-only descriptions of actual layouts, shared by all callable objects.
pub type DebugTypes = BTreeMap<PersistentExactTypeId, DebugType>;

#[derive(Clone, Debug)]
pub struct DebugType {
    pub name: String,
    pub size: u64,
    pub align: u64,
    pub kind: DebugTypeKind,
}

#[derive(Clone, Debug)]
pub enum DebugTypeRef {
    Source(PersistentExactTypeId),
    /// Physical subobjects such as an object header or an enum variant do not
    /// introduce new semantic type identities.
    Inline(Box<DebugType>),
}

#[derive(Clone, Debug)]
pub enum DebugTypeKind {
    Basic(DebugEncoding),
    Pointer(DebugTypeRef),
    Record(Vec<DebugField>),
    Array {
        element: DebugTypeRef,
        count: u64,
    },
    Enumeration {
        signed: bool,
        variants: Vec<(String, u64)>,
    },
    Function {
        parameters: Vec<DebugTypeRef>,
        result: Option<DebugTypeRef>,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum DebugEncoding {
    Boolean,
    Signed,
    Unsigned,
    Float,
    Unicode,
    Address,
}

#[derive(Clone, Debug)]
pub struct DebugField {
    pub name: String,
    pub offset: u64,
    pub ty: DebugTypeRef,
    pub artificial: bool,
}
