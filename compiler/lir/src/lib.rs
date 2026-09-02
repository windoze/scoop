//! LIR definitions and LIR meta: the data channel between LIR and codegen.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4 and
//! `docs/milestone2/DESIGN.md` section 2.4.
//!
//! LIR contains nothing Scoop-specific: functions are basic blocks of
//! explicit instructions over locals and temporaries, and codegen
//! translates them mechanically. All locals are stack slots (alloca);
//! SSA construction is left to LLVM's mem2reg.

use std::num::NonZeroU64;

use la_arena::{Arena, Idx};

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

#[derive(Debug)]
pub struct Module {
    pub globals: Arena<Global>,
    /// Struct definitions with complete physical layouts (indexed by
    /// `StructDefId`; ids align with MIR struct ids).
    pub structs: Arena<StructDef>,
    /// Enum definitions with fixed representations (indexed by
    /// `EnumDefId`).
    pub enums: Arena<EnumDef>,
    pub functions: Vec<Function>,
    /// Native declarations and C-bridge descriptions, transposed from MIR.
    /// ABI-refined references can only be minted while declarations are
    /// inserted into this registry.
    pub extern_functions: ExternFunctions,
    /// C data imports accessed only through generated get/set/address bridges.
    pub native_globals: Arena<NativeGlobal>,
    /// Typed bridge entities used by native-global access records. Separate id
    /// families make get/set/address roles impossible to interchange.
    pub native_global_bridges: NativeGlobalBridges,
    /// Inbound C trampolines that adapt a native signature to a NoGC
    /// Scoop storage-ABI bridge.
    pub callback_bridges: Arena<CallbackBridge>,
    /// GC-aware managed callback trampolines. These are disjoint from M12's
    /// NoGC static callback bridges at the type level.
    pub foreign_callback_bridges: Arena<ForeignCallbackBridge>,
    /// Symbol of the entry function (`scoop_main`).
    pub entry_symbol: String,
    pub meta: LirMeta,
}

#[derive(Debug)]
pub struct CallbackBridge {
    pub source_name: String,
    pub bridge_symbol: String,
    pub trampoline_symbol: String,
    pub params: Vec<CType>,
    pub return_type: CType,
}

#[derive(Debug)]
pub struct ForeignCallbackBridge {
    pub adapter_symbol: String,
    pub trampoline_symbol: String,
    pub signature_symbol: String,
    pub params: Vec<CType>,
    pub return_type: CType,
    pub context_index: u32,
    pub mode: ForeignCallbackMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackMode {
    Reusable,
    OneShot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackOperation {
    Retain { out: TempId, callback: Value },
    Release { callback: Value },
    State { out: TempId, callback: Value },
    Failure { out: TempId, callback: Value },
}

impl ForeignCallbackOperation {
    pub fn callback(self) -> Value {
        match self {
            Self::Retain { callback, .. }
            | Self::Release { callback }
            | Self::State { callback, .. }
            | Self::Failure { callback, .. } => callback,
        }
    }

    pub fn out(self) -> Option<TempId> {
        match self {
            Self::Retain { out, .. } | Self::State { out, .. } | Self::Failure { out, .. } => {
                Some(out)
            }
            Self::Release { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct NativeGlobal {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub ty: LirType,
    pub c_type: CType,
    pub thread_local: bool,
    pub access: NativeGlobalAccess,
}

#[derive(Debug, Default)]
pub struct NativeGlobalBridges {
    pub gets: Arena<NativeGlobalGetBridge>,
    pub sets: Arena<NativeGlobalSetBridge>,
    pub addresses: Arena<NativeGlobalAddressBridge>,
}

#[derive(Debug)]
pub struct NativeGlobalGetBridge {
    pub symbol: String,
}

#[derive(Debug)]
pub struct NativeGlobalSetBridge {
    pub symbol: String,
}

#[derive(Debug)]
pub struct NativeGlobalAddressBridge {
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeGlobalAccess {
    ReadOnly {
        get: NativeGlobalGetBridgeId,
        address: NativeGlobalAddressBridgeId,
    },
    Mutable {
        get: NativeGlobalGetBridgeId,
        set: NativeGlobalSetBridgeId,
        address: NativeGlobalAddressBridgeId,
    },
}

impl NativeGlobalAccess {
    pub fn get(self) -> NativeGlobalGetBridgeId {
        match self {
            Self::ReadOnly { get, .. } | Self::Mutable { get, .. } => get,
        }
    }

    pub fn set(self) -> Option<NativeGlobalSetBridgeId> {
        match self {
            Self::ReadOnly { .. } => None,
            Self::Mutable { set, .. } => Some(set),
        }
    }

    pub fn address(self) -> NativeGlobalAddressBridgeId {
        match self {
            Self::ReadOnly { address, .. } | Self::Mutable { address, .. } => address,
        }
    }
}

mod externs;
pub use externs::*;
#[derive(Debug)]
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

/// A local variable's stack slot.
#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: LirType,
}

/// A temporary SSA-ish value produced by an instruction.
#[derive(Debug)]
pub struct Temp {
    pub ty: LirType,
}

#[derive(Debug)]
pub struct Function {
    /// Whether codegen must attach the GC strategy. Polls are explicit LIR
    /// instructions with their own typed root plans.
    pub gc_effect: GcEffect,
    pub symbol: String,
    /// Parameter types; arguments are SSA values (`Value::Param`).
    pub params: Vec<LirType>,
    pub return_ty: LirType,
    /// Function-local call entities. Targets may contain local SSA operands
    /// (for dispatch tables), so their ids are scoped to this function.
    pub call_targets: CallTargets,
    pub locals: Arena<Local>,
    pub temps: Arena<Temp>,
    pub blocks: Arena<BasicBlock>,
    /// Entry block; every function has exactly one.
    pub entry: BlockId,
}

mod calls;
pub use calls::*;

impl Function {
    /// The type of a value in this function.
    pub fn value_ty(&self, globals: &Arena<Global>, value: Value) -> LirType {
        match value {
            Value::Local(id) => self.locals[id].ty.clone(),
            Value::Temp(id) => self.temps[id].ty.clone(),
            Value::Param(index) => self.params[index as usize].clone(),
            Value::IntConst(_) => LirType::I64,
            Value::BoolConst(_) => LirType::I1,
            Value::NullPointer(kind) => LirType::Ptr(kind),
            Value::TypeDescriptor(_) => METADATA_PTR,
            Value::RootScan(_) => METADATA_PTR,
            Value::Global(id) => LirType::Ptr(globals[id].address_kind),
        }
    }
}

#[derive(Debug)]
pub struct BasicBlock {
    pub name: String,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

/// A value usable as an instruction operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Value {
    /// Contents of a local's stack slot (loaded implicitly).
    Local(LocalId),
    /// A function parameter (0-based).
    Param(u32),
    Temp(TempId),
    IntConst(i64),
    BoolConst(bool),
    NullPointer(PointerKind),
    /// Address of a local or external TypeDescriptor.
    TypeDescriptor(TypeDescriptorRef),
    /// Address of one complete function-local recursive root scan program.
    RootScan(RootScanId),
    /// Address of a global constant.
    Global(GlobalId),
}

#[derive(Debug)]
pub enum Instruction {
    /// `out = <op> lhs, rhs` (integer or boolean; the type is on `out`).
    BinOp {
        out: TempId,
        op: BinOp,
        lhs: Value,
        rhs: Value,
    },
    /// `out = -operand` / `out = !operand`.
    UnaryOp {
        out: TempId,
        op: UnOp,
        operand: Value,
    },
    /// Build an aggregate value (struct / tuple construction, or the
    /// Unit value with zero elements).
    MakeAggregate {
        out: TempId,
        elements: Vec<Value>,
    },
    /// Extract field / element `index` from an aggregate value.
    ExtractValue {
        out: TempId,
        aggregate: Value,
        index: u32,
    },
    /// Load a typed value from `object + offset`. Heap objects use
    /// natural byte-aligned field offsets after their 16-byte header;
    /// the same primitive reads fixed-offset runtime metadata such as
    /// the TypeDescriptor pointer and vtable pointer.
    HeapLoad {
        out: TempId,
        object: Value,
        offset: u64,
    },
    /// Acquire-load a 64-bit synthetic state word from managed storage.
    AtomicLoad {
        out: TempId,
        object: Value,
        offset: u64,
    },
    /// `store value -> local`'s stack slot.
    Store {
        local: LocalId,
        value: Value,
    },
    GlobalLoad {
        out: TempId,
        global: GlobalId,
    },
    GlobalStore {
        global: GlobalId,
        value: Value,
    },
    GlobalAddress {
        out: TempId,
        global: GlobalId,
    },
    NativeGlobalLoad {
        out: TempId,
        global: NativeGlobalId,
        safepoint: SafepointId,
        roots: NativeSafeRootSet,
    },
    NativeGlobalStore {
        global: NativeGlobalId,
        value: Value,
        safepoint: SafepointId,
        roots: NativeSafeRootSet,
    },
    NativeGlobalAddress {
        out: TempId,
        global: NativeGlobalId,
        safepoint: SafepointId,
        roots: NativeSafeRootSet,
    },
    /// Store a typed value at the byte address `object + offset`.
    /// Class fields use their natural layout offsets, base-class fields
    /// first. `offset` must be at least 16 so the object header cannot
    /// be overwritten.
    HeapStore {
        object: Value,
        offset: u64,
        value: Value,
    },
    /// Release-store a 64-bit synthetic state word in managed storage.
    AtomicStore {
        object: Value,
        offset: u64,
        value: Value,
    },
    /// Acq_rel/acquire compare-exchange of a 64-bit synthetic state word.
    /// `out` receives the observed old word.
    AtomicCompareExchange {
        out: TempId,
        object: Value,
        offset: u64,
        expected: Value,
        replacement: Value,
    },
    /// Materialize the address of a module function as an opaque code
    /// pointer. It is metadata, not a managed reference.
    FunctionAddress {
        out: TempId,
        symbol: String,
    },
    ForeignCallbackRegister {
        out: TempId,
        bridge: ForeignCallbackBridgeId,
        closure: Value,
    },
    ForeignCallbackOperation(ForeignCallbackOperation),
    IntToPtr {
        out: TempId,
        value: Value,
    },
    PtrToInt {
        out: TempId,
        value: Value,
    },
    RawLoad {
        out: TempId,
        pointer: Value,
        align: u64,
    },
    RawStore {
        pointer: Value,
        value: Value,
        align: u64,
    },
    /// Byte-wise pointer displacement. `bytes` may be negative.
    PtrOffset {
        out: TempId,
        pointer: Value,
        bytes: Value,
    },
    LocalAddress {
        out: TempId,
        local: LocalId,
    },
    /// Direct or dispatch call. The target atomically owns destination,
    /// physical ABI signature, return convention and effect.
    Call {
        site: CallSite,
    },
    /// Explicit entry/back-edge poll. NoGC functions cannot contain one.
    ManagedPoll {
        site: ManagedPollSite,
    },
    /// Call that may throw (inside a `try`): control transfers to
    /// `normal` on success and to the `unwind` landing pad on a
    /// thrown exception (spec 11.7, runtime spec 5). Terminator-like:
    /// must be the last instruction of its block; the block's
    /// `Terminator` is the redundant `Br(normal)` — it only restates
    /// the normal successor for dump readability, and codegen uses
    /// this instruction as the LLVM terminator without emitting the
    /// branch.
    Invoke {
        site: InvokeSite,
    },
    /// Catch-all landing pad. It captures the opaque unwind record and
    /// raw exception pointer but does not begin the catch; `BeginCatch`
    /// is explicit in the ordinary dispatch block.
    LandingPad {
        record: TempId,
        raw: TempId,
    },
    /// Cleanup-only landing pad (no catch clause). It captures the same
    /// record/raw pair for cleanup chaining or `Terminator::Resume`.
    CleanupPad {
        record: TempId,
        raw: TempId,
    },
    /// Begin handling the raw exception and return its Scoop object.
    BeginCatch {
        out: TempId,
        raw: Value,
    },
    /// End the innermost active catch (`__cxa_end_catch()`).
    EndCatch,
    /// Throw an exception object (does not return). Terminator-like:
    /// must be the last instruction of its block, which ends
    /// `Unreachable` (the same shape as the M3 trap path).
    Throw {
        exception: Value,
    },
    /// Array operations. Every instruction names the complete concrete array
    /// metadata it consumes; no downstream pass recovers it from operand or
    /// result layouts.
    /// Allocate an array object and store the elements in order.
    ArrayAlloc {
        out: TempId,
        elements: Vec<Value>,
        array_type: ArrayTypeId,
        safepoint: SafepointId,
        live: StatepointLiveSet,
    },
    /// `array.size` (result `I64`).
    ArrayLen {
        out: TempId,
        operand: Value,
        array_type: ArrayTypeId,
    },
    /// Bounds-checked element read (traps out of range).
    ArrayGet {
        out: TempId,
        array: Value,
        index: Value,
        array_type: ArrayTypeId,
    },
    /// Bounds-checked element write (traps out of range).
    ArraySet {
        array: Value,
        index: Value,
        value: Value,
        array_type: ArrayTypeId,
    },
    /// `Array(m)` / `MutableArray(a)` conversion (memcpy snapshot).
    ArrayClone {
        out: TempId,
        operand: Value,
        /// Target array application (`Array<T>` or `MutableArray<T>`).
        array_type: ArrayTypeId,
        safepoint: SafepointId,
        live: StatepointLiveSet,
    },
    /// Enum operations. The representation (niche pointer or tagged
    /// union) is fixed by `EnumDef::repr`, so codegen translates these
    /// mechanically.
    /// Construct a variant value.
    EnumWrap {
        out: TempId,
        enum_id: EnumDefId,
        variant: u32,
        fields: Vec<Value>,
    },
    /// Read the variant tag (result `I64`; niche: null test).
    EnumTag {
        out: TempId,
        enum_id: EnumDefId,
        operand: Value,
    },
    /// Read field `index` of variant `variant`; only emitted on paths
    /// where the tag is known to match.
    EnumField {
        out: TempId,
        enum_id: EnumDefId,
        variant: u32,
        index: u32,
        operand: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    SDiv,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug)]
pub enum Terminator {
    Br(BlockId),
    CondBr {
        cond: Value,
        then_block: BlockId,
        else_block: BlockId,
    },
    /// `value` is absent exactly for void (Unit-returning) functions.
    Return {
        value: Option<Value>,
    },
    /// Continue unwinding with the record produced by `CleanupPad`.
    Resume {
        exception: Value,
    },
    /// After a noreturn call (e.g. the trap function).
    Unreachable,
}

mod dump;

pub use dump::dump;

#[cfg(test)]
mod tests {
    use super::{
        CExternFunction, CExternFunctionRef, CallDestination, CallTarget, CallTargets,
        CallingConvention, DirectCallSignature, ExternFunctionDeclaration, ExternFunctions,
        GcEffect, LirType, ManagedCallDestination, ManagedRuntimeFunction,
        NativeBorrowedCallDestination, NativeBorrowedResultPublication, NativeBorrowedResultRoot,
        NativeSafeCallDestination, NonEmptyRefScan, RefScan, ResultStorage, ScoopExternFunction,
        ScoopExternFunctionRef, TypedCall, TypedCallView, Value, VoidCallSignature,
    };

    #[test]
    fn non_empty_ref_scan_rejects_programs_without_references() {
        assert!(NonEmptyRefScan::new(RefScan::None).is_none());
        assert!(NonEmptyRefScan::new(RefScan::References(Vec::new())).is_none());
        assert!(
            NonEmptyRefScan::new(RefScan::Sequence(vec![
                RefScan::None,
                RefScan::References(Vec::new()),
            ]))
            .is_none()
        );
    }

    #[test]
    fn non_empty_ref_scan_accepts_nested_references() {
        let scan = RefScan::Sequence(vec![
            RefScan::None,
            RefScan::Sequence(vec![RefScan::References(vec![16])]),
        ]);
        let scan = NonEmptyRefScan::new(scan).expect("nested reference makes the scan non-empty");

        assert_eq!(
            scan.as_ref_scan(),
            &RefScan::Sequence(vec![
                RefScan::None,
                RefScan::Sequence(vec![RefScan::References(vec![16])]),
            ])
        );
    }

    #[test]
    fn typed_targets_atomically_bind_protocol_return_convention_and_signature() {
        let mut targets = CallTargets::default();
        let void_signature = targets.void_signatures.alloc(VoidCallSignature {
            params: Vec::new(),
            calling_convention: CallingConvention::Cdecl,
        });
        let void_target = targets.managed_targets.void.alloc(CallTarget {
            destination: ManagedCallDestination::runtime(ManagedRuntimeFunction::GcCollect),
            signature: void_signature,
        });
        let void_call = TypedCall::Void {
            target: void_target,
            args: Vec::new(),
        };

        let direct_signature = targets.direct_signatures.alloc(DirectCallSignature {
            params: vec![LirType::I64],
            result: LirType::I64,
            result_scan: RefScan::None,
            calling_convention: CallingConvention::Cdecl,
        });
        let direct_target = targets.managed_targets.direct.alloc(CallTarget {
            destination: ManagedCallDestination::local(super::LocalFunctionId::from_u32(7)),
            signature: direct_signature,
        });
        let direct_call = TypedCall::Direct {
            target: direct_target,
            out: super::TempId::from_raw(la_arena::RawIdx::from_u32(0)),
            args: vec![Value::IntConst(1)],
        };

        let void_view = targets.typed_call_view(
            &void_call,
            &targets.managed_targets,
            ManagedCallDestination::view,
        );
        let direct_view = targets.typed_call_view(
            &direct_call,
            &targets.managed_targets,
            ManagedCallDestination::view,
        );

        assert!(matches!(
            void_view,
            TypedCallView::Void {
                destination: CallDestination::Runtime(_),
                signature: VoidCallSignature { params, .. },
                ..
            } if params.is_empty()
        ));
        assert!(matches!(
            direct_view,
            TypedCallView::Direct {
                destination: CallDestination::Local(_),
                signature: DirectCallSignature { params, result: LirType::I64, .. },
                ..
            } if params == &[LirType::I64]
        ));
    }

    #[test]
    fn native_borrowed_result_publication_is_sealed_with_return_convention() {
        let mut functions = ExternFunctions::default();
        let function = functions.alloc_scoop(ScoopExternFunction {
            declaration: ExternFunctionDeclaration {
                source_name: "borrowed".to_string(),
                native_symbol: "native_borrowed".to_string(),
                library: "test".to_string(),
                calling_convention: CallingConvention::Cdecl,
                params: Vec::new(),
                return_type: super::MANAGED_PTR,
            },
            gc_effect: GcEffect::Managed,
        });
        let destination = NativeBorrowedCallDestination::extern_function(function);
        let mut targets = CallTargets::default();
        let result_scan = RefScan::References(vec![0]);

        let direct_signature = targets.direct_signatures.alloc(DirectCallSignature {
            params: Vec::new(),
            result: super::MANAGED_PTR,
            result_scan: result_scan.clone(),
            calling_convention: CallingConvention::Cdecl,
        });
        let direct_target = targets.native_borrowed_targets.direct.alloc(CallTarget {
            destination,
            signature: direct_signature,
        });
        let direct_storage = super::LocalId::from_raw(la_arena::RawIdx::from_u32(0));
        let direct = targets.bind_native_borrowed_call(
            TypedCall::Direct {
                target: direct_target,
                out: super::TempId::from_raw(la_arena::RawIdx::from_u32(0)),
                args: Vec::new(),
            },
            NativeBorrowedResultRoot::Rooted {
                storage: direct_storage,
            },
        );
        let direct = direct.view(&targets);
        assert!(matches!(direct.call, TypedCallView::Direct { .. }));
        assert!(matches!(
            direct.result,
            NativeBorrowedResultPublication::DirectRooted { storage, scan }
                if storage == direct_storage && scan.as_ref_scan() == &result_scan
        ));

        let indirect_storage = super::LocalId::from_raw(la_arena::RawIdx::from_u32(1));
        let indirect_signature =
            targets
                .indirect_result_signatures
                .alloc(super::IndirectResultCallSignature {
                    params: Vec::new(),
                    result: ResultStorage {
                        ty: super::MANAGED_PTR,
                        scan: result_scan.clone(),
                    },
                    calling_convention: CallingConvention::Cdecl,
                });
        let indirect_target = targets
            .native_borrowed_targets
            .indirect_result
            .alloc(CallTarget {
                destination,
                signature: indirect_signature,
            });
        let indirect = targets.bind_native_borrowed_call(
            TypedCall::IndirectResult {
                target: indirect_target,
                storage: indirect_storage,
                args: Vec::new(),
            },
            NativeBorrowedResultRoot::Rooted {
                storage: indirect_storage,
            },
        );
        let indirect = indirect.view(&targets);
        assert!(matches!(
            indirect.result,
            NativeBorrowedResultPublication::IndirectResultRooted { storage, scan }
                if storage == indirect_storage && scan.as_ref_scan() == &result_scan
        ));
    }

    #[test]
    fn extern_references_are_refined_by_abi_before_entering_call_targets() {
        let mut functions = ExternFunctions::default();
        let c_ref: CExternFunctionRef = functions.alloc_c(CExternFunction {
            declaration: ExternFunctionDeclaration {
                source_name: "c".to_string(),
                native_symbol: "c".to_string(),
                library: "test".to_string(),
                calling_convention: CallingConvention::Cdecl,
                params: Vec::new(),
                return_type: LirType::Void,
            },
            bridge_symbol: "c_bridge".to_string(),
            params: Vec::new(),
            return_type: super::CType::Unit,
        });
        let scoop_ref: ScoopExternFunctionRef = functions.alloc_scoop(ScoopExternFunction {
            declaration: ExternFunctionDeclaration {
                source_name: "scoop".to_string(),
                native_symbol: "scoop".to_string(),
                library: "test".to_string(),
                calling_convention: CallingConvention::Cdecl,
                params: Vec::new(),
                return_type: LirType::Void,
            },
            gc_effect: GcEffect::Managed,
        });

        let c = c_ref.declaration();
        let scoop = scoop_ref.declaration();
        assert!(matches!(
            functions[c].kind,
            super::ExternFunctionKind::C { .. }
        ));
        assert!(matches!(
            functions[scoop].kind,
            super::ExternFunctionKind::Scoop { .. }
        ));
        assert_eq!(
            NativeSafeCallDestination::extern_function(c_ref).view(),
            CallDestination::Extern(c)
        );
        assert_eq!(
            NativeBorrowedCallDestination::extern_function(scoop_ref).view(),
            CallDestination::Extern(scoop)
        );
    }
}
