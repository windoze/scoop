use super::*;

pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
    pub size: u64,
    pub align: u64,
    pub c_layout: Option<CLayout>,
    pub interior_mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub ty: LirType,
    pub layout: FieldLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldLayout {
    pub offset: u64,
    /// Alignment that a load/store of this field may claim. Packed layouts
    /// cap this independently of the field type's natural alignment.
    pub access_align: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CLayout {
    pub aligned: u8,
    pub packed: u8,
}

/// Per-Cone LIR metadata (impl spec 2.4): type layouts.
#[derive(Debug)]
pub struct LirMeta {
    /// Non-optional identities selected from typed intrinsic declarations.
    pub well_known_layouts: WellKnownLayouts,
    pub well_known_type_descriptors: WellKnownTypeDescriptors,
    /// Every fully specialized intrinsic `Array<T>` / `MutableArray<T>`
    /// application. Array instructions carry an `ArrayTypeId`; codegen never
    /// reconstructs nominal array identity or GC metadata from value layouts.
    pub arrays: Arena<ArrayType>,
    pub layouts: Arena<Layout>,
    /// Locally emitted TypeDescriptors. Every semantic edge uses a typed ref;
    /// `symbol` is only a final link attribute.
    pub type_descriptors: Arena<TypeDescriptor>,
    /// Cross-Cone descriptors are declared but not initialized by this Cone.
    pub external_type_descriptors: Arena<ExternalTypeDescriptor>,
    /// Cross-Cone callables referenced from local dispatch tables.
    pub external_callables: Arena<ExternalCallable>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WellKnownLayouts {
    pub string: LayoutId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WellKnownTypeDescriptors {
    pub string: TypeDescriptorRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayKind {
    Immutable,
    Mutable,
}

/// Complete LIR metadata for one concrete intrinsic array application.
/// `element_size` / `element_align` are fixed by lir-lower rather than
/// recomputed from LLVM ABI queries in codegen.
#[derive(Debug)]
pub struct ArrayType {
    pub kind: ArrayKind,
    pub element: LirType,
    pub element_size: u64,
    pub element_align: u64,
    /// The descriptor owns the recursive repeated-element scan program.
    pub type_descriptor: TypeDescriptorRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeDescriptorRef {
    Local(TypeDescriptorId),
    External(ExternalTypeDescriptorId),
}

#[derive(Debug)]
pub struct ExternalTypeDescriptor {
    /// Final linker spelling; never used as semantic identity.
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallableRef {
    Local(LocalFunctionId),
    Runtime(RuntimeFunction),
    External(ExternalCallableId),
}

#[derive(Debug)]
pub struct ExternalCallable {
    /// Final linker spelling; the typed arena id is the semantic identity.
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DispatchEntry {
    pub callable: CallableRef,
}

/// Everything codegen needs to emit one `ScoopTypeDescriptor`
/// global (see runtime/include/scoop_rt.h for the field order).
#[derive(Debug)]
pub struct TypeDescriptor {
    /// Human-readable type name used in metadata dumps.
    pub name: String,
    /// Global symbol, e.g. `scoop_td_Point`.
    pub symbol: String,
    /// Runtime-visible identity selected by lir-lower. Codegen does not infer
    /// it from arena position or descriptor category.
    pub runtime_type_id: u64,
    pub size: u64,
    pub align: u64,
    pub scan: TypeDescriptorScan,
    /// Classes reference their base descriptor; root/reference-key entities
    /// have no parent. The absence is emitted as a metadata-provenance null.
    pub parent: Option<TypeDescriptorRef>,
    pub vtable: Vec<DispatchEntry>,
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDescriptorScan {
    /// Recursive GC scan program for a fixed-size object payload.
    Fixed(RefScan),
    /// Recursive scan for one inline array element, repeated at `stride`.
    ArrayElement { stride: u64, scan: RefScan },
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface: TypeDescriptorRef,
    pub slots: Vec<DispatchEntry>,
}

#[derive(Debug)]
pub struct Layout {
    pub name: String,
    pub size: u64,
    pub align: u64,
    pub fields: Vec<FieldLayout>,
    pub c_layout: Option<CLayout>,
    pub interior_mutable: bool,
    pub kind: LayoutKind,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LayoutKind {
    Plain {
        scan: RefScan,
    },
    /// Enum layouts retain their identity while exposing one fixed scan
    /// program for the complete physical value. Tagged-enum scans never
    /// branch on the tag: inactive ref-bearing slots are zero-filled.
    Enum {
        scan: RefScan,
    },
    /// Compiler representation selected by the typed intrinsic application
    /// in MIR. Generic family variants carry the fully lowered element type.
    Intrinsic(IntrinsicTypeRepresentation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    Int,
    UInt,
    Boolean,
    String,
}

/// Recursive, layout-complete description of references in an inline
/// value. Every offset is relative to the base supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefScan {
    None,
    References(Vec<u64>),
    Sequence(Vec<RefScan>),
}

impl RefScan {
    pub fn dump(&self) -> String {
        match self {
            Self::None => "none".to_string(),
            Self::References(offsets) => format!("refs{offsets:?}"),
            Self::Sequence(parts) => format!(
                "seq({})",
                parts.iter().map(Self::dump).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    pub fn contains_reference(&self) -> bool {
        match self {
            Self::None => false,
            Self::References(offsets) => !offsets.is_empty(),
            Self::Sequence(parts) => parts.iter().any(Self::contains_reference),
        }
    }
}

/// A recursive scan program that is guaranteed to visit at least one managed
/// reference. This is the only scan representation accepted by caller roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyRefScan(RefScan);

impl NonEmptyRefScan {
    pub fn new(scan: RefScan) -> Option<Self> {
        scan.contains_reference().then_some(Self(scan))
    }

    pub fn as_ref_scan(&self) -> &RefScan {
        &self.0
    }

    pub fn dump(&self) -> String {
        self.0.dump()
    }
}

#[derive(Debug)]
pub struct Global {
    pub symbol: String,
    /// Provenance of the address produced by `Value::Global`.
    pub address_kind: PointerKind,
    /// Complete recursive scan program for the writable global storage.
    /// Immortal object and C-string globals explicitly carry `None`.
    pub scan: RefScan,
    pub init: GlobalInit,
}

/// An enum definition with its representation fixed by lir-lower.
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    pub repr: EnumRepr,
    /// Recursive scan program for one inline value of this enum type.
    pub scan: RefScan,
}

#[derive(Debug)]
pub enum EnumRepr {
    /// Niche optimization (spec 7.4): only an enum structurally isomorphic to
    /// `Option<ref/Ptr/FunPtr>` may use it — exactly one empty variant and one
    /// single pointer-represented payload variant. Names and order do not
    /// matter. Whether the word is managed remains a payload property.
    Niche {
        /// Index of the payload-carrying variant.
        payload_variant: u32,
    },
    /// A tag followed by one optional shared pure-value payload slot and
    /// one disjoint slot for every variant that contains managed refs.
    Tagged {
        variants: Vec<EnumVariantRepr>,
        size: u64,
        align: u64,
    },
}

/// Physical storage assigned to one tagged-enum variant.
#[derive(Debug)]
pub struct EnumVariantRepr {
    pub fields: Vec<EnumFieldRepr>,
    pub slot_offset: u64,
    pub slot_size: u64,
    pub slot_align: u64,
    /// Copied from the fully specialized MIR variant. Only GC-free
    /// variants may share the pure-value payload slot.
    pub gc_free: bool,
}

/// The complete physical representation of one tagged-enum field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumFieldRepr {
    pub ty: LirType,
    /// Enum-relative offset, including the variant's slot offset.
    pub offset: u64,
}

#[derive(Debug)]
pub enum GlobalInit {
    /// A `ScoopString` constant: header points at `STRING_TD_SYMBOL`.
    StringConst(String),
    /// A NUL-terminated C string (e.g. trap messages).
    CString(String),
    Storage {
        ty: LirType,
        initializer: ConstantValue,
        thread_local: bool,
    },
}

#[derive(Debug)]
pub enum ConstantValue {
    Int(i64),
    Bool(bool),
    NullPointer(PointerKind),
    Struct {
        struct_id: StructDefId,
        fields: Vec<ConstantValue>,
    },
}
