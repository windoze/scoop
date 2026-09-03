use super::*;

pub type GlobalId = Idx<Global>;
pub type LocalId = Idx<Local>;
pub type TempId = Idx<Temp>;
pub type BlockId = Idx<BasicBlock>;
pub type EnumDefId = Idx<EnumDef>;
pub type StructDefId = Idx<StructDef>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type NativeGlobalId = Idx<NativeGlobal>;
pub type NativeGlobalGetBridgeId = Idx<NativeGlobalGetBridge>;
pub type NativeGlobalSetBridgeId = Idx<NativeGlobalSetBridge>;
pub type NativeGlobalAddressBridgeId = Idx<NativeGlobalAddressBridge>;
pub type CallbackBridgeId = Idx<CallbackBridge>;
pub type ForeignCallbackBridgeId = Idx<ForeignCallbackBridge>;
pub type ArrayTypeId = Idx<ArrayType>;
pub type DispatchSlotId = Idx<DispatchSlot>;
pub type RootScanId = Idx<RefScan>;
pub type LayoutId = Idx<Layout>;
pub type TypeDescriptorId = Idx<TypeDescriptor>;
pub type ExternalTypeDescriptorId = Idx<ExternalTypeDescriptor>;
pub type ExternalCallableId = Idx<ExternalCallable>;

/// Typed index into `Module::functions`. Functions remain in emission order,
/// while call destinations no longer use their symbols as semantic identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalFunctionId(u32);

impl LocalFunctionId {
    pub const fn from_u32(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn into_u32(self) -> u32 {
        self.0
    }
}

/// Deterministic, nonzero, image-wide identity of one actual safepoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SafepointId(NonZeroU64);

impl SafepointId {
    pub fn new(raw: u64) -> Option<Self> {
        NonZeroU64::new(raw).map(Self)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Symbol of the TypeDescriptor global for `String` (runtime spec 2.2).
pub const STRING_TD_SYMBOL: &str = "scoop_td_String";

/// Runtime trap (M3: `!!` on `None`; M8: real exceptions).
pub const TRAP_SYMBOL: &str = "scoop_rt_trap";

/// Runtime array clone (spec 10.4 conversions).
pub const ARRAY_CLONE_SYMBOL: &str = "scoop_rt_array_clone";

/// A type after layout resolution: maps directly onto LLVM types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LirType {
    Void,
    I1,
    I64,
    Ptr(PointerKind),
    /// Opaque Itanium EH landing-pad record (`{ ptr, i32 }` in LLVM).
    /// It is produced by exception pads and may be consumed by `Resume`.
    ExceptionRecord,
    /// Tuple / Unit values: an LLVM literal struct.
    Aggregate(Vec<LirType>),
    /// A named Scoop struct. Its exact physical layout is carried by the
    /// module's `StructDef`, so packed and over-aligned layouts cannot be
    /// erased into an anonymous natural aggregate.
    Struct(StructDefId),
    /// An enum value; the representation is fixed by
    /// `EnumDef::repr` (niche pointer or tagged union, spec 7.4).
    Enum(EnumDefId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerKind {
    /// A GC-traced reference to a managed heap object.
    Managed,
    /// A native address that the GC must neither trace nor relocate.
    Raw,
    /// An executable function address.
    Code,
    /// An immortal runtime descriptor or dispatch-table address.
    Metadata,
}

pub const MANAGED_PTR: LirType = LirType::Ptr(PointerKind::Managed);
pub const RAW_PTR: LirType = LirType::Ptr(PointerKind::Raw);
pub const CODE_PTR: LirType = LirType::Ptr(PointerKind::Code);
pub const METADATA_PTR: LirType = LirType::Ptr(PointerKind::Metadata);

impl PointerKind {
    pub fn dump(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Raw => "raw",
            Self::Code => "code",
            Self::Metadata => "metadata",
        }
    }
}

impl LirType {
    pub fn dump(&self) -> String {
        match self {
            LirType::Void => "void".to_string(),
            LirType::I1 => "i1".to_string(),
            LirType::I64 => "i64".to_string(),
            LirType::Ptr(kind) => format!("ptr<{}>", kind.dump()),
            LirType::ExceptionRecord => "exception_record".to_string(),
            LirType::Aggregate(elements) => {
                let inner: Vec<String> = elements.iter().map(LirType::dump).collect();
                format!("{{{}}}", inner.join(", "))
            }
            LirType::Struct(id) => format!("struct{}", id.into_raw()),
            LirType::Enum(id) => format!("enum{}", id.into_raw()),
        }
    }
}
