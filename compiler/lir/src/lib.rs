//! LIR definitions and LIR meta: the data channel between LIR and codegen.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4 and
//! `docs/milestone2/DESIGN.md` section 2.4.
//!
//! LIR contains nothing Scoop-specific: functions are basic blocks of
//! explicit instructions over locals and temporaries, and codegen
//! translates them mechanically. All locals are stack slots (alloca);
//! SSA construction is left to LLVM's mem2reg.

use la_arena::{Arena, Idx};

pub type GlobalId = Idx<Global>;
pub type LocalId = Idx<Local>;
pub type TempId = Idx<Temp>;
pub type BlockId = Idx<BasicBlock>;
pub type EnumDefId = Idx<EnumDef>;
pub type StructDefId = Idx<StructDef>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type NativeGlobalId = Idx<NativeGlobal>;
pub type CallbackBridgeId = Idx<CallbackBridge>;
pub type ForeignCallbackBridgeId = Idx<ForeignCallbackBridge>;
pub type ArrayTypeId = Idx<ArrayType>;
pub type VoidCallSignatureId = Idx<VoidCallSignature>;
pub type DirectCallSignatureId = Idx<DirectCallSignature>;
pub type IndirectResultCallSignatureId = Idx<IndirectResultCallSignature>;
pub type VoidCallTargetId = Idx<VoidCallTarget>;
pub type DirectCallTargetId = Idx<DirectCallTarget>;
pub type IndirectResultCallTargetId = Idx<IndirectResultCallTarget>;
pub type DispatchSlotId = Idx<DispatchSlot>;
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
    pub extern_functions: Arena<ExternFunction>,
    /// C data imports accessed only through generated get/set/address bridges.
    pub native_globals: Arena<NativeGlobal>,
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
    Retain,
    Release,
    State,
    Failure,
}

#[derive(Debug)]
pub struct NativeGlobal {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub ty: LirType,
    pub c_type: CType,
    pub mutable: bool,
    pub thread_local: bool,
    pub get_bridge_symbol: String,
    pub set_bridge_symbol: Option<String>,
    pub address_bridge_symbol: String,
}

#[derive(Debug)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub calling_convention: CallingConvention,
    pub params: Vec<LirType>,
    pub return_type: LirType,
    pub kind: ExternFunctionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug)]
pub enum ExternFunctionKind {
    C {
        bridge_symbol: String,
        params: Vec<CType>,
        return_type: CType,
    },
    Scoop {
        gc_effect: GcEffect,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CType {
    Unit,
    Int,
    UInt,
    Boolean,
    Pointer,
    FunctionPointer {
        params: Vec<CType>,
        return_type: Box<CType>,
    },
    Struct(StructDefId),
}

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
}

#[derive(Debug)]
pub struct Global {
    pub symbol: String,
    /// Provenance of the address produced by `Value::Global`.
    pub address_kind: PointerKind,
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
    pub fields: Vec<LirType>,
    /// Enum-relative field offsets, including the variant's slot offset.
    pub field_offsets: Vec<u64>,
    pub slot_offset: u64,
    pub slot_size: u64,
    pub slot_align: u64,
    /// Copied from the fully specialized MIR variant. Only GC-free
    /// variants may share the pure-value payload slot.
    pub gc_free: bool,
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
    /// Whether codegen must attach the GC strategy and safepoint polls.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallEffect {
    ManagedSafepoint,
    NoGc,
    NativeSafe,
    NativeBorrowed,
}

#[derive(Debug, Default)]
pub struct CallTargets {
    pub void_signatures: Arena<VoidCallSignature>,
    pub direct_signatures: Arena<DirectCallSignature>,
    pub indirect_result_signatures: Arena<IndirectResultCallSignature>,
    pub void_targets: Arena<VoidCallTarget>,
    pub direct_targets: Arena<DirectCallTarget>,
    pub indirect_result_targets: Arena<IndirectResultCallTarget>,
    pub dispatch_slots: Arena<DispatchSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoidCallSignature {
    pub params: Vec<LirType>,
    pub calling_convention: CallingConvention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCallSignature {
    pub params: Vec<LirType>,
    pub result: LirType,
    /// Recursive scan of a direct result. Native transitions use it to
    /// publish result storage before re-entering managed code.
    pub result_scan: RefScan,
    pub calling_convention: CallingConvention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndirectResultCallSignature {
    pub params: Vec<LirType>,
    pub result: ResultStorage,
    pub calling_convention: CallingConvention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultStorage {
    pub ty: LirType,
    pub scan: RefScan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallDestination {
    Local(LocalFunctionId),
    Runtime(RuntimeFunction),
    Extern(ExternFunctionId),
    Dispatch { table: Value, slot: DispatchSlotId },
}

#[derive(Debug)]
pub struct VoidCallTarget {
    pub destination: CallDestination,
    pub signature: VoidCallSignatureId,
    pub effect: CallEffect,
}

#[derive(Debug)]
pub struct DirectCallTarget {
    pub destination: CallDestination,
    pub signature: DirectCallSignatureId,
    pub effect: CallEffect,
}

#[derive(Debug)]
pub struct IndirectResultCallTarget {
    pub destination: CallDestination,
    pub signature: IndirectResultCallSignatureId,
    pub effect: CallEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchSlot {
    pub kind: DispatchKind,
    pub index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchKind {
    Virtual,
    Interface,
    Closure,
    FunctionBridge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeFunction {
    Alloc,
    Box,
    IsInstance,
    ITableLookup,
    Pin,
    Unpin,
    GetHandle,
    ReleaseHandle,
    GcCollect,
    GcStats,
    MaterializeException,
    IntToString,
    BoolToString,
    StringConcat,
    StringEq,
    Trap,
    Throw,
    Rethrow,
}

impl RuntimeFunction {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Alloc => "scoop_rt_alloc",
            Self::Box => "scoop_rt_box",
            Self::IsInstance => "scoop_rt_is_instance",
            Self::ITableLookup => "scoop_rt_itable_lookup",
            Self::Pin => "scoop_rt_pin",
            Self::Unpin => "scoop_rt_unpin",
            Self::GetHandle => "scoop_rt_get_handle",
            Self::ReleaseHandle => "scoop_rt_release_handle",
            Self::GcCollect => "scoop_rt_gc_collect",
            Self::GcStats => "scoop_rt_gc_stats",
            Self::MaterializeException => "scoop_rt_materialize_exception",
            Self::IntToString => "scoop_rt_int_to_string",
            Self::BoolToString => "scoop_rt_bool_to_string",
            Self::StringConcat => "scoop_rt_string_concat",
            Self::StringEq => "scoop_rt_string_eq",
            Self::Trap => "scoop_rt_trap",
            Self::Throw => "scoop_rt_throw",
            Self::Rethrow => "scoop_rt_rethrow",
        }
    }
}

#[derive(Debug)]
pub enum CallSite {
    Void {
        target: VoidCallTargetId,
        args: Vec<Value>,
    },
    Direct {
        target: DirectCallTargetId,
        out: TempId,
        args: Vec<Value>,
    },
    IndirectResult {
        target: IndirectResultCallTargetId,
        storage: LocalId,
        args: Vec<Value>,
    },
}

impl CallSite {
    pub fn args(&self) -> &[Value] {
        match self {
            Self::Void { args, .. }
            | Self::Direct { args, .. }
            | Self::IndirectResult { args, .. } => args,
        }
    }

    pub fn destination(&self, targets: &CallTargets) -> CallDestination {
        match *self {
            Self::Void { target, .. } => targets.void_targets[target].destination,
            Self::Direct { target, .. } => targets.direct_targets[target].destination,
            Self::IndirectResult { target, .. } => {
                targets.indirect_result_targets[target].destination
            }
        }
    }

    pub fn effect(&self, targets: &CallTargets) -> CallEffect {
        match *self {
            Self::Void { target, .. } => targets.void_targets[target].effect,
            Self::Direct { target, .. } => targets.direct_targets[target].effect,
            Self::IndirectResult { target, .. } => targets.indirect_result_targets[target].effect,
        }
    }

    pub fn result_scan<'a>(&self, targets: &'a CallTargets) -> &'a RefScan {
        match *self {
            Self::Void { .. } => &RefScan::None,
            Self::Direct { target, .. } => {
                let signature = targets.direct_targets[target].signature;
                &targets.direct_signatures[signature].result_scan
            }
            Self::IndirectResult { target, .. } => {
                let signature = targets.indirect_result_targets[target].signature;
                &targets.indirect_result_signatures[signature].result.scan
            }
        }
    }

    pub fn direct_out(&self) -> Option<TempId> {
        match *self {
            Self::Direct { out, .. } => Some(out),
            Self::Void { .. } | Self::IndirectResult { .. } => None,
        }
    }

    pub fn result_storage(&self) -> Option<LocalId> {
        match *self {
            Self::IndirectResult { storage, .. } => Some(storage),
            Self::Void { .. } | Self::Direct { .. } => None,
        }
    }
}

/// A value whose address is published in a compiler caller-root frame.
/// Constants and globals cannot appear here: constants have no storage and
/// managed globals are already roots in their own right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallerRootSource {
    Param(u32),
    Local(LocalId),
    Temp(TempId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerRoot {
    pub source: CallerRootSource,
    /// Non-empty recursive scan program relative to the source's storage.
    pub scan: RefScan,
}

impl CallerRootSource {
    fn dump(self) -> String {
        match self {
            Self::Param(index) => format!("param{index}"),
            Self::Local(id) => format!("local{}", id.into_raw()),
            Self::Temp(id) => format!("t{}", id.into_raw()),
        }
    }
}

impl CallEffect {
    fn dump(self) -> &'static str {
        match self {
            Self::ManagedSafepoint => "managed-safepoint",
            Self::NoGc => "no-gc",
            Self::NativeSafe => "native-safe",
            Self::NativeBorrowed => "native-borrowed",
        }
    }
}

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
        roots: Vec<CallerRoot>,
    },
    NativeGlobalStore {
        global: NativeGlobalId,
        value: Value,
        roots: Vec<CallerRoot>,
    },
    NativeGlobalAddress {
        out: TempId,
        global: NativeGlobalId,
        roots: Vec<CallerRoot>,
    },
    /// A nounwind native call. C ABI operands are storage pointers to the
    /// generated bridge; Scoop ABI operands retain their ordinary typed ABI.
    NativeCall {
        site: CallSite,
        /// Values live across this native transition. Codegen spills SSA
        /// sources and publishes each storage with its recursive scan.
        roots: Vec<CallerRoot>,
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
    ForeignCallbackOperation {
        out: Option<TempId>,
        operation: ForeignCallbackOperation,
        callback: Value,
    },
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
    /// Call that may throw (inside a `try`): control transfers to
    /// `normal` on success and to the `unwind` landing pad on a
    /// thrown exception (spec 11.7, runtime spec 5). Terminator-like:
    /// must be the last instruction of its block; the block's
    /// `Terminator` is the redundant `Br(normal)` — it only restates
    /// the normal successor for dump readability, and codegen uses
    /// this instruction as the LLVM terminator without emitting the
    /// branch.
    Invoke {
        site: CallSite,
        normal: BlockId,
        unwind: BlockId,
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

/// Indented text dump for golden tests (`scoopc build --emit=lir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst(value) => {
                out.push_str(&format!("  global @{} = {:?}\n", global.symbol, value));
            }
            GlobalInit::CString(value) => {
                out.push_str(&format!("  global @{} = c{:?}\n", global.symbol, value));
            }
            GlobalInit::Storage {
                ty, thread_local, ..
            } => out.push_str(&format!(
                "  {} @{} : {}\n",
                if *thread_local {
                    "thread_local"
                } else {
                    "global"
                },
                global.symbol,
                ty.dump()
            )),
        }
    }
    for (id, global) in module.native_globals.iter() {
        out.push_str(&format!(
            "  native_global{} {} @{} : {}{}\n",
            id.into_raw(),
            global.source_name,
            global.native_symbol,
            global.ty.dump(),
            if global.thread_local {
                " thread_local"
            } else {
                ""
            }
        ));
    }
    for (_, def) in module.enums.iter() {
        match &def.repr {
            EnumRepr::Niche { payload_variant } => out.push_str(&format!(
                "  enum {} niche(payload_variant={})\n",
                def.name, payload_variant
            )),
            EnumRepr::Tagged {
                variants,
                size,
                align,
            } => {
                let variants: Vec<String> = variants
                    .iter()
                    .map(|variant| {
                        let inner: Vec<String> = variant.fields.iter().map(LirType::dump).collect();
                        format!(
                            "({})@{}+{}{}",
                            inner.join(", "),
                            variant.slot_offset,
                            variant.slot_size,
                            if variant.gc_free { "" } else { ":refs" }
                        )
                    })
                    .collect();
                out.push_str(&format!(
                    "  enum {} tagged size={} align={} variants={}\n",
                    def.name,
                    size,
                    align,
                    variants.join(" ")
                ));
            }
        }
    }
    for (id, extern_) in module.extern_functions.iter() {
        let params = extern_
            .params
            .iter()
            .map(LirType::dump)
            .collect::<Vec<_>>()
            .join(", ");
        let kind = match &extern_.kind {
            ExternFunctionKind::C { bridge_symbol, .. } => {
                format!("c bridge=@{bridge_symbol} gc-leaf nounwind")
            }
            ExternFunctionKind::Scoop { gc_effect } => format!(
                "scoop {} nounwind",
                if *gc_effect == GcEffect::NoGc {
                    "gc-leaf"
                } else {
                    "managed"
                }
            ),
        };
        let library = if extern_.library.is_empty() {
            String::new()
        } else {
            format!(" lib={}", extern_.library)
        };
        out.push_str(&format!(
            "  extern ef{} {} @{}({}) -> {} <{}{}>\n",
            id.into_raw(),
            extern_.source_name,
            extern_.native_symbol,
            params,
            extern_.return_type.dump(),
            kind,
            library
        ));
    }
    for (id, callback) in module.callback_bridges.iter() {
        out.push_str(&format!(
            "  callback cb{} {} @{} -> @{}\n",
            id.into_raw(),
            callback.source_name,
            callback.bridge_symbol,
            callback.trampoline_symbol
        ));
    }
    for function in &module.functions {
        let params: Vec<String> = function.params.iter().map(LirType::dump).collect();
        out.push_str(&format!(
            "  fun @{}({}) -> {}{}\n",
            function.symbol,
            params.join(", "),
            function.return_ty.dump(),
            if function.gc_effect == GcEffect::NoGc {
                " <no-gc>"
            } else {
                ""
            }
        ));
        for (id, local) in function.locals.iter() {
            out.push_str(&format!(
                "    local %{} {}: {}\n",
                id.into_raw(),
                local.name,
                local.ty.dump()
            ));
        }
        for (block_id, block) in function.blocks.iter() {
            let _ = block_id;
            out.push_str(&format!("  block {}\n", block.name));
            for instruction in &block.instructions {
                dump_instruction(function, instruction, &mut out);
            }
            match &block.terminator {
                Terminator::Br(target) => {
                    out.push_str(&format!("    br @{}\n", block_name(function, *target)))
                }
                Terminator::CondBr {
                    cond,
                    then_block,
                    else_block,
                } => out.push_str(&format!(
                    "    cbr {} then @{} else @{}\n",
                    value_name(*cond),
                    block_name(function, *then_block),
                    block_name(function, *else_block)
                )),
                Terminator::Return { value } => match value {
                    Some(value) => out.push_str(&format!("    ret {}\n", value_name(*value))),
                    None => out.push_str("    ret\n"),
                },
                Terminator::Resume { exception } => {
                    out.push_str(&format!("    resume {}\n", value_name(*exception)))
                }
                Terminator::Unreachable => out.push_str("    unreachable\n"),
            }
        }
    }
    for (id, td) in module.meta.type_descriptors.iter() {
        let reference = TypeDescriptorRef::Local(id);
        if reference == module.meta.well_known_type_descriptors.string
            || module
                .meta
                .arrays
                .iter()
                .any(|(_, array)| array.type_descriptor == reference)
        {
            continue;
        }
        let parent = td
            .parent
            .map(type_descriptor_ref_name)
            .unwrap_or_else(|| "none".to_string());
        let vtable = td
            .vtable
            .iter()
            .map(|entry| callable_ref_name(entry.callable))
            .collect::<Vec<_>>()
            .join(", ");
        let itables = td
            .itables
            .iter()
            .map(|record| {
                let slots = record
                    .slots
                    .iter()
                    .map(|entry| callable_ref_name(entry.callable))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}:[{slots}]", type_descriptor_ref_name(record.interface))
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "  td td{} {} @{} type-id={} size={} parent={} vtable=[{}] itables=[{}]\n",
            id.into_raw(),
            td.name,
            td.symbol,
            td.runtime_type_id,
            td.size,
            parent,
            vtable,
            itables,
        ));
    }
    for (id, array) in module.meta.arrays.iter() {
        let TypeDescriptorRef::Local(descriptor_id) = array.type_descriptor else {
            unreachable!("a local array application owns a local descriptor")
        };
        let descriptor = &module.meta.type_descriptors[descriptor_id];
        let TypeDescriptorScan::ArrayElement { scan, .. } = &descriptor.scan else {
            unreachable!("an array descriptor owns an element scan")
        };
        out.push_str(&format!(
            "  array-type array{} {} kind={} element={} size={} align={} scan={} td={}\n",
            id.into_raw(),
            descriptor.name,
            match array.kind {
                ArrayKind::Immutable => "immutable",
                ArrayKind::Mutable => "mutable",
            },
            array.element.dump(),
            array.element_size,
            array.element_align,
            scan.dump(),
            type_descriptor_ref_name(array.type_descriptor),
        ));
    }
    // Keep the textual dump stable while the typed intrinsic metadata remains
    // available directly on `LirMeta`: String historically appeared first,
    // and the unused UInt scalar was omitted. Tests that validate the intrinsic
    // contract inspect the typed fields instead of reconstructing it from text.
    let string_layout = module.meta.well_known_layouts.string;
    for (_layout_id, layout) in
        std::iter::once((string_layout, &module.meta.layouts[string_layout]))
            .chain(module.meta.layouts.iter().filter(|(_, layout)| {
                matches!(
                    layout.kind,
                    LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Int)
                )
            }))
            .chain(module.meta.layouts.iter().filter(|(_, layout)| {
                matches!(
                    layout.kind,
                    LayoutKind::Intrinsic(IntrinsicTypeRepresentation::Boolean)
                )
            }))
            .chain(module.meta.layouts.iter().filter(|(_, layout)| {
                !matches!(
                    layout.kind,
                    LayoutKind::Intrinsic(
                        IntrinsicTypeRepresentation::Int
                            | IntrinsicTypeRepresentation::UInt
                            | IntrinsicTypeRepresentation::Boolean
                            | IntrinsicTypeRepresentation::String
                    )
                )
            }))
    {
        match &layout.kind {
            LayoutKind::Plain { scan } => match scan {
                RefScan::None => out.push_str(&format!(
                    "  layout {} size={} align={} refs=[]\n",
                    layout.name, layout.size, layout.align
                )),
                RefScan::References(offsets) => out.push_str(&format!(
                    "  layout {} size={} align={} refs={offsets:?}\n",
                    layout.name, layout.size, layout.align
                )),
                _ => out.push_str(&format!(
                    "  layout {} size={} align={} scan={}\n",
                    layout.name,
                    layout.size,
                    layout.align,
                    scan.dump()
                )),
            },
            LayoutKind::Enum { scan } => out.push_str(&format!(
                "  layout {} size={} align={} enum-scan={}\n",
                layout.name,
                layout.size,
                layout.align,
                scan.dump()
            )),
            LayoutKind::Intrinsic(
                IntrinsicTypeRepresentation::Int
                | IntrinsicTypeRepresentation::UInt
                | IntrinsicTypeRepresentation::Boolean
                | IntrinsicTypeRepresentation::String,
            ) => out.push_str(&format!(
                "  layout {} size={} align={} refs=[]\n",
                layout.name, layout.size, layout.align
            )),
        }
        if let Some(c_layout) = layout.c_layout {
            let fields = layout
                .fields
                .iter()
                .map(|field| format!("{}@{}", field.offset, field.access_align))
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!(
                "  layout-meta {} c-layout(aligned={},packed={}) fields=[{}] interior-mutable={}\n",
                layout.name, c_layout.aligned, c_layout.packed, fields, layout.interior_mutable
            ));
        } else if layout.interior_mutable {
            out.push_str(&format!(
                "  layout-meta {} interior-mutable=true\n",
                layout.name
            ));
        }
    }
    out.push_str(&format!("  entry @{}\n", module.entry_symbol));
    out
}

fn block_name(function: &Function, id: BlockId) -> String {
    function.blocks[id].name.clone()
}

fn value_name(value: Value) -> String {
    match value {
        Value::Local(id) => format!("local{}", id.into_raw()),
        Value::Param(index) => format!("param{index}"),
        Value::Temp(id) => format!("t{}", id.into_raw()),
        Value::IntConst(value) => format!("{value}"),
        Value::BoolConst(value) => format!("{value}"),
        Value::NullPointer(kind) => format!("null<{}>", kind.dump()),
        Value::TypeDescriptor(reference) => type_descriptor_ref_name(reference),
        Value::Global(id) => format!("global{}", id.into_raw()),
    }
}

fn type_descriptor_ref_name(reference: TypeDescriptorRef) -> String {
    match reference {
        TypeDescriptorRef::Local(id) => format!("td{}", id.into_raw()),
        TypeDescriptorRef::External(id) => format!("external-td{}", id.into_raw()),
    }
}

fn callable_ref_name(reference: CallableRef) -> String {
    match reference {
        CallableRef::Local(id) => format!("local-fn{}", id.into_u32()),
        CallableRef::Runtime(function) => format!("runtime@{}", function.symbol()),
        CallableRef::External(id) => format!("external-fn{}", id.into_raw()),
    }
}

fn call_destination_name(function: &Function, destination: CallDestination) -> String {
    match destination {
        CallDestination::Local(id) => format!("local-fn{}", id.into_u32()),
        CallDestination::Runtime(runtime) => format!("runtime @{}", runtime.symbol()),
        CallDestination::Extern(id) => format!("extern{}", id.into_raw()),
        CallDestination::Dispatch { table, slot } => {
            let slot = function.call_targets.dispatch_slots[slot];
            format!(
                "dispatch[{:?}:{}] {}",
                slot.kind,
                slot.index,
                value_name(table)
            )
        }
    }
}

fn call_site_name(function: &Function, site: &CallSite) -> String {
    let targets = &function.call_targets;
    let args = site
        .args()
        .iter()
        .map(|arg| value_name(*arg))
        .collect::<Vec<_>>()
        .join(", ");
    match *site {
        CallSite::Void { target, .. } => {
            let target_value = &targets.void_targets[target];
            let signature = &targets.void_signatures[target_value.signature];
            let params = signature
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "void-target{} sig=void{} ({params}) effect={} {}({args})",
                target.into_raw(),
                target_value.signature.into_raw(),
                target_value.effect.dump(),
                call_destination_name(function, target_value.destination),
            )
        }
        CallSite::Direct { target, out, .. } => {
            let target_value = &targets.direct_targets[target];
            let signature = &targets.direct_signatures[target_value.signature];
            let params = signature
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "t{} = direct-target{} sig=direct{} ({params}) -> {} effect={} {}({args})",
                out.into_raw(),
                target.into_raw(),
                target_value.signature.into_raw(),
                signature.result.dump(),
                target_value.effect.dump(),
                call_destination_name(function, target_value.destination),
            )
        }
        CallSite::IndirectResult {
            target, storage, ..
        } => {
            let target_value = &targets.indirect_result_targets[target];
            let signature = &targets.indirect_result_signatures[target_value.signature];
            let params = signature
                .params
                .iter()
                .map(LirType::dump)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "local{} = indirect-result-target{} sig=indirect{} (sret {}, {params}) effect={} {}({args})",
                storage.into_raw(),
                target.into_raw(),
                target_value.signature.into_raw(),
                signature.result.ty.dump(),
                target_value.effect.dump(),
                call_destination_name(function, target_value.destination),
            )
        }
    }
}

fn dump_instruction(function: &Function, instruction: &Instruction, buf: &mut String) {
    match instruction {
        Instruction::BinOp { out, op, lhs, rhs } => buf.push_str(&format!(
            "    t{} = {:?} {}, {} : {}\n",
            out.into_raw(),
            op,
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::UnaryOp { out, op, operand } => buf.push_str(&format!(
            "    t{} = {:?} {} : {}\n",
            out.into_raw(),
            op,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::MakeAggregate { out, elements } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = aggregate ({}) : {}\n",
                out.into_raw(),
                elements.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ExtractValue {
            out,
            aggregate,
            index,
        } => buf.push_str(&format!(
            "    t{} = extract {}, {} : {}\n",
            out.into_raw(),
            value_name(*aggregate),
            index,
            function.temps[*out].ty.dump()
        )),
        Instruction::HeapLoad {
            out,
            object,
            offset,
        } => buf.push_str(&format!(
            "    t{} = heap_load {} +{} : {}\n",
            out.into_raw(),
            value_name(*object),
            offset,
            function.temps[*out].ty.dump()
        )),
        Instruction::AtomicLoad {
            out,
            object,
            offset,
        } => buf.push_str(&format!(
            "    t{} = atomic_load acquire {} +{} : {}\n",
            out.into_raw(),
            value_name(*object),
            offset,
            function.temps[*out].ty.dump()
        )),
        Instruction::GlobalLoad { out, global } => buf.push_str(&format!(
            "    t{} = global_load global{} : {}\n",
            out.into_raw(),
            global.into_raw(),
            function.temps[*out].ty.dump()
        )),
        Instruction::GlobalStore { global, value } => buf.push_str(&format!(
            "    global_store global{}, {}\n",
            global.into_raw(),
            value_name(*value)
        )),
        Instruction::GlobalAddress { out, global } => buf.push_str(&format!(
            "    t{} = global_address global{}\n",
            out.into_raw(),
            global.into_raw()
        )),
        Instruction::NativeGlobalLoad { out, global, roots } => buf.push_str(&format!(
            "    t{} = native_global_load ng{} roots={} : {}\n",
            out.into_raw(),
            global.into_raw(),
            roots.len(),
            function.temps[*out].ty.dump()
        )),
        Instruction::NativeGlobalStore {
            global,
            value,
            roots,
        } => buf.push_str(&format!(
            "    native_global_store ng{}, {} roots={}\n",
            global.into_raw(),
            value_name(*value),
            roots.len()
        )),
        Instruction::NativeGlobalAddress { out, global, roots } => buf.push_str(&format!(
            "    t{} = native_global_address ng{} roots={}\n",
            out.into_raw(),
            global.into_raw(),
            roots.len()
        )),
        Instruction::HeapStore {
            object,
            offset,
            value,
        } => buf.push_str(&format!(
            "    heap_store {} +{} {}\n",
            value_name(*object),
            offset,
            value_name(*value)
        )),
        Instruction::AtomicStore {
            object,
            offset,
            value,
        } => buf.push_str(&format!(
            "    atomic_store release {} +{} {}\n",
            value_name(*object),
            offset,
            value_name(*value)
        )),
        Instruction::AtomicCompareExchange {
            out,
            object,
            offset,
            expected,
            replacement,
        } => buf.push_str(&format!(
            "    t{} = atomic_cmpxchg acq_rel/acquire {} +{} expected={} replacement={} : {}\n",
            out.into_raw(),
            value_name(*object),
            offset,
            value_name(*expected),
            value_name(*replacement),
            function.temps[*out].ty.dump()
        )),
        Instruction::FunctionAddress { out, symbol } => buf.push_str(&format!(
            "    t{} = function_address @{} : ptr\n",
            out.into_raw(),
            symbol
        )),
        Instruction::ForeignCallbackRegister {
            out,
            bridge,
            closure,
        } => buf.push_str(&format!(
            "    t{} = foreign_callback_register fcb{} {} : {}\n",
            out.into_raw(),
            bridge.into_raw(),
            value_name(*closure),
            function.temps[*out].ty.dump()
        )),
        Instruction::ForeignCallbackOperation {
            out,
            operation,
            callback,
        } => match out {
            Some(out) => buf.push_str(&format!(
                "    t{} = foreign_callback_{:?} {} : {}\n",
                out.into_raw(),
                operation,
                value_name(*callback),
                function.temps[*out].ty.dump()
            )),
            None => buf.push_str(&format!(
                "    foreign_callback_{:?} {}\n",
                operation,
                value_name(*callback)
            )),
        },
        Instruction::IntToPtr { out, value } => buf.push_str(&format!(
            "    t{} = int_to_ptr {} : ptr\n",
            out.into_raw(),
            value_name(*value)
        )),
        Instruction::PtrToInt { out, value } => buf.push_str(&format!(
            "    t{} = ptr_to_int {} : i64\n",
            out.into_raw(),
            value_name(*value)
        )),
        Instruction::RawLoad {
            out,
            pointer,
            align,
        } => buf.push_str(&format!(
            "    t{} = raw_load {} align {} : {}\n",
            out.into_raw(),
            value_name(*pointer),
            align,
            function.temps[*out].ty.dump()
        )),
        Instruction::RawStore {
            pointer,
            value,
            align,
        } => buf.push_str(&format!(
            "    raw_store {} {} align {}\n",
            value_name(*pointer),
            value_name(*value),
            align
        )),
        Instruction::PtrOffset {
            out,
            pointer,
            bytes,
        } => buf.push_str(&format!(
            "    t{} = ptr_offset {} {} : ptr\n",
            out.into_raw(),
            value_name(*pointer),
            value_name(*bytes)
        )),
        Instruction::LocalAddress { out, local } => buf.push_str(&format!(
            "    t{} = local_address local{} : ptr\n",
            out.into_raw(),
            local.into_raw()
        )),
        Instruction::Store { local, value } => buf.push_str(&format!(
            "    store {} -> local{}\n",
            value_name(*value),
            local.into_raw()
        )),
        Instruction::NativeCall { site, roots } => {
            let roots = if roots.is_empty() {
                String::new()
            } else {
                let roots = roots
                    .iter()
                    .map(|root| {
                        let source = root.source.dump();
                        match &root.scan {
                            RefScan::References(offsets) if offsets == &[0] => source,
                            scan => format!("{source}:{}", scan.dump()),
                        }
                    })
                    .collect::<Vec<_>>();
                format!(" roots=[{}]", roots.join(", "))
            };
            let result_scan = site.result_scan(&function.call_targets);
            let result_root = if *result_scan == RefScan::None {
                String::new()
            } else {
                format!(" result-root={}", result_scan.dump())
            };
            buf.push_str(&format!(
                "    native_call {}{}{}\n",
                call_site_name(function, site),
                roots,
                result_root
            ));
        }
        Instruction::Call { site } => {
            buf.push_str(&format!("    call {}\n", call_site_name(function, site)));
        }
        Instruction::Invoke {
            site,
            normal,
            unwind,
        } => {
            buf.push_str(&format!(
                "    invoke {} normal @{} unwind @{}\n",
                call_site_name(function, site),
                block_name(function, *normal),
                block_name(function, *unwind)
            ));
        }
        Instruction::LandingPad { record, raw } => buf.push_str(&format!(
            "    (t{}, t{}) = landingpad : ({}, {})\n",
            record.into_raw(),
            raw.into_raw(),
            function.temps[*record].ty.dump(),
            function.temps[*raw].ty.dump()
        )),
        Instruction::CleanupPad { record, raw } => buf.push_str(&format!(
            "    (t{}, t{}) = cleanup_pad : ({}, {})\n",
            record.into_raw(),
            raw.into_raw(),
            function.temps[*record].ty.dump(),
            function.temps[*raw].ty.dump()
        )),
        Instruction::BeginCatch { out, raw } => buf.push_str(&format!(
            "    t{} = begin_catch {} : {}\n",
            out.into_raw(),
            value_name(*raw),
            function.temps[*out].ty.dump()
        )),
        Instruction::EndCatch => buf.push_str("    end_catch\n"),
        Instruction::Throw { exception } => {
            buf.push_str(&format!("    throw {}\n", value_name(*exception)))
        }
        Instruction::ArrayAlloc {
            out,
            elements,
            array_type,
        } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = array_alloc array{} ({}) : {}\n",
                out.into_raw(),
                array_type.into_raw(),
                elements.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ArrayLen {
            out,
            operand,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_len array{} {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArrayGet {
            out,
            array,
            index,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_get array{} {} {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*array),
            value_name(*index),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArraySet {
            array,
            index,
            value,
            array_type,
        } => buf.push_str(&format!(
            "    array_set array{} {} {} {}\n",
            array_type.into_raw(),
            value_name(*array),
            value_name(*index),
            value_name(*value)
        )),
        Instruction::ArrayClone {
            out,
            operand,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_clone array{} {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumWrap {
            out,
            enum_id,
            variant,
            fields,
        } => {
            let fields: Vec<String> = fields.iter().map(|f| value_name(*f)).collect();
            buf.push_str(&format!(
                "    t{} = enum_wrap e{} v{} ({}) : {}\n",
                out.into_raw(),
                enum_id.into_raw(),
                variant,
                fields.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::EnumTag {
            out,
            enum_id,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_tag e{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumField {
            out,
            enum_id,
            variant,
            index,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_field e{} v{} f{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            variant,
            index,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
    }
}
