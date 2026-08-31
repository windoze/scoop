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
    /// An array object: pointer to `{ td, i64 size, inline elements }`
    /// (spec 10.1). The payload is the element layout.
    Array(Box<LirType>),
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
            LirType::Array(inner) => format!("[{}]", inner.dump()),
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
    pub layouts: Vec<Layout>,
    /// TypeDescriptors to emit (runtime spec 2.2): classes, boxed
    /// value types, and interfaces (symbols serve as itable keys).
    /// Emission order is significant: `parent` / interface symbols
    /// must refer to entries in this list (or to
    /// `STRING_TD_SYMBOL`).
    pub type_descriptors: Vec<TypeDescriptor>,
}

/// Everything codegen needs to emit one `ScoopTypeDescriptor`
/// global (see runtime/include/scoop_rt.h for the field order).
#[derive(Debug)]
pub struct TypeDescriptor {
    /// Name for dumps and the default `toString`.
    pub name: String,
    /// Global symbol, e.g. `scoop_td_Point`.
    pub symbol: String,
    /// `type_id` (codegen assigns small integers, starting after the
    /// built-ins).
    pub size: u64,
    pub align: u64,
    /// Recursive GC scan program for the object payload. Unlike a flat
    /// offset list, this preserves tagged enums nested in aggregates.
    pub scan: RefScan,
    /// Symbol of the parent TypeDescriptor (classes: base class;
    /// boxed value types / interfaces: none).
    pub parent: Option<String>,
    /// vtable slot symbols (functions or `scoop_rt_any_*`).
    pub vtable: Vec<String>,
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface_symbol: String,
    pub slots: Vec<String>,
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
    /// Array layouts carry the scan program for one inline element;
    /// codegen wraps it in the runtime array descriptor.
    Array {
        element_scan: RefScan,
    },
    /// Enum layouts keep one scan program per variant. Their offsets
    /// are relative to the enum value and therefore compose when the
    /// enum is nested in another aggregate.
    Enum {
        variants: Vec<VariantLayout>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct VariantLayout {
    pub scan: RefScan,
}

/// Recursive, layout-complete description of references in an inline
/// value. Every offset is relative to the base supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefScan {
    None,
    References(Vec<u64>),
    Sequence(Vec<RefScan>),
    TaggedEnum {
        tag_offset: u64,
        variants: Vec<RefScan>,
    },
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
            Self::TaggedEnum {
                tag_offset,
                variants,
            } => format!(
                "enum@{tag_offset}[{}]",
                variants
                    .iter()
                    .map(Self::dump)
                    .collect::<Vec<_>>()
                    .join(", ")
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
}

#[derive(Debug)]
pub enum EnumRepr {
    /// Niche optimization (spec 7.4): two variants, one without
    /// payload, the other a single pointer-represented field — the value
    /// is one pointer word and the `None`-style variant is 0. Whether that
    /// word is a managed reference remains a property of the payload type.
    Niche {
        /// Index of the payload-carrying variant.
        payload_variant: u32,
    },
    /// `{ i64 tag, [N x i8] payload }`.
    Tagged {
        /// Field types of each variant, in declaration order.
        variants: Vec<Vec<LirType>>,
        payload_size: u64,
        payload_align: u64,
    },
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
    NullPtr,
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
pub enum NativeCallEffect {
    NativeSafe,
    NativeBorrowed,
}

impl NativeCallEffect {
    fn dump(self) -> &'static str {
        match self {
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
            Value::NullPtr => LirType::Ptr(PointerKind::Raw),
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
#[derive(Debug, Clone, Copy)]
pub enum Value {
    /// Contents of a local's stack slot (loaded implicitly).
    Local(LocalId),
    /// A function parameter (0-based).
    Param(u32),
    Temp(TempId),
    IntConst(i64),
    BoolConst(bool),
    NullPtr,
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
    },
    NativeGlobalStore {
        global: NativeGlobalId,
        value: Value,
    },
    NativeGlobalAddress {
        out: TempId,
        global: NativeGlobalId,
    },
    /// A nounwind native call. C ABI operands are storage pointers to the
    /// generated bridge; Scoop ABI operands retain their ordinary typed ABI.
    NativeCall {
        out: Option<TempId>,
        function: ExternFunctionId,
        effect: NativeCallEffect,
        args: Vec<Value>,
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
    /// Materialize the address of a module function as an opaque code
    /// pointer. It is metadata, not a managed reference.
    FunctionAddress {
        out: TempId,
        symbol: String,
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
    /// Direct call. `out` is `None` exactly when the callee returns
    /// void; runtime functions with results produce a Temp of the
    /// result type.
    Call {
        out: Option<TempId>,
        symbol: String,
        args: Vec<Value>,
    },
    /// Indirect call through a function table (vtable / itable
    /// dispatch, impl spec 2.9): `table` is a `ptr` to the first slot,
    /// the callee is `table[slot]`. Arguments include the receiver.
    CallIndirect {
        out: Option<TempId>,
        table: Value,
        slot: u32,
        args: Vec<Value>,
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
        out: Option<TempId>,
        symbol: String,
        args: Vec<Value>,
        normal: BlockId,
        unwind: BlockId,
    },
    /// Indirect variant of `Invoke` (same terminator convention).
    InvokeIndirect {
        out: Option<TempId>,
        table: Value,
        slot: u32,
        args: Vec<Value>,
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
    /// Array operations. The element layout is the `Array(...)` type
    /// of the array operand (or of `out` for `ArrayAlloc`).
    /// Allocate an array object and store the elements in order.
    ArrayAlloc {
        out: TempId,
        elements: Vec<Value>,
        /// Scan program for one element, relative to its first byte.
        element_scan: RefScan,
    },
    /// `array.size` (result `I64`).
    ArrayLen {
        out: TempId,
        operand: Value,
    },
    /// Bounds-checked element read (traps out of range).
    ArrayGet {
        out: TempId,
        array: Value,
        index: Value,
    },
    /// Bounds-checked element write (traps out of range).
    ArraySet {
        array: Value,
        index: Value,
        value: Value,
    },
    /// `Array(m)` / `MutableArray(a)` conversion (memcpy snapshot).
    ArrayClone {
        out: TempId,
        operand: Value,
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
                payload_size,
                payload_align,
            } => {
                let variants: Vec<String> = variants
                    .iter()
                    .map(|fields| {
                        let inner: Vec<String> = fields.iter().map(LirType::dump).collect();
                        format!("({})", inner.join(", "))
                    })
                    .collect();
                out.push_str(&format!(
                    "  enum {} tagged size={} align={} variants={}\n",
                    def.name,
                    payload_size,
                    payload_align,
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
    for td in &module.meta.type_descriptors {
        out.push_str(&format!(
            "  td {} @{} size={} vtable={} itables={}\n",
            td.name,
            td.symbol,
            td.size,
            td.vtable.len(),
            td.itables.len()
        ));
    }
    for layout in &module.meta.layouts {
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
            LayoutKind::Array { element_scan } => match element_scan {
                RefScan::None => out.push_str(&format!(
                    "  layout {} size={} align={} array(element_is_ref=false)\n",
                    layout.name, layout.size, layout.align
                )),
                RefScan::References(offsets) if offsets == &[0] => out.push_str(&format!(
                    "  layout {} size={} align={} array(element_is_ref=true)\n",
                    layout.name, layout.size, layout.align
                )),
                _ => out.push_str(&format!(
                    "  layout {} size={} align={} array(scan={})\n",
                    layout.name,
                    layout.size,
                    layout.align,
                    element_scan.dump()
                )),
            },
            LayoutKind::Enum { variants } => {
                let simple_offsets = variants
                    .iter()
                    .map(|variant| match &variant.scan {
                        RefScan::None => Some(Vec::new()),
                        RefScan::References(offsets) => Some(offsets.clone()),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(offsets) = simple_offsets {
                    out.push_str(&format!(
                        "  layout {} size={} align={} enum-refs={offsets:?}\n",
                        layout.name, layout.size, layout.align
                    ));
                } else {
                    out.push_str(&format!(
                        "  layout {} size={} align={} enum-scan=[{}]\n",
                        layout.name,
                        layout.size,
                        layout.align,
                        variants
                            .iter()
                            .map(|variant| variant.scan.dump())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
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
        Value::NullPtr => "null".to_string(),
        Value::Global(id) => format!("global{}", id.into_raw()),
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
        Instruction::NativeGlobalLoad { out, global } => buf.push_str(&format!(
            "    t{} = native_global_load ng{} : {}\n",
            out.into_raw(),
            global.into_raw(),
            function.temps[*out].ty.dump()
        )),
        Instruction::NativeGlobalStore { global, value } => buf.push_str(&format!(
            "    native_global_store ng{}, {}\n",
            global.into_raw(),
            value_name(*value)
        )),
        Instruction::NativeGlobalAddress { out, global } => buf.push_str(&format!(
            "    t{} = native_global_address ng{}\n",
            out.into_raw(),
            global.into_raw()
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
        Instruction::FunctionAddress { out, symbol } => buf.push_str(&format!(
            "    t{} = function_address @{} : ptr\n",
            out.into_raw(),
            symbol
        )),
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
        Instruction::NativeCall {
            out,
            function: extern_id,
            effect,
            args,
        } => {
            let args = args.iter().map(|arg| value_name(*arg)).collect::<Vec<_>>();
            match out {
                Some(temp) => buf.push_str(&format!(
                    "    t{} = native_call[{}] extern{}({}) : {}\n",
                    temp.into_raw(),
                    effect.dump(),
                    extern_id.into_raw(),
                    args.join(", "),
                    function.temps[*temp].ty.dump()
                )),
                None => buf.push_str(&format!(
                    "    native_call[{}] extern{}({})\n",
                    effect.dump(),
                    extern_id.into_raw(),
                    args.join(", ")
                )),
            }
        }
        Instruction::Call { out, symbol, args } => {
            let args: Vec<String> = args.iter().map(|a| value_name(*a)).collect();
            match out {
                Some(temp) => buf.push_str(&format!(
                    "    t{} = call @{}({}) : {}\n",
                    temp.into_raw(),
                    symbol,
                    args.join(", "),
                    function.temps[*temp].ty.dump()
                )),
                None => buf.push_str(&format!("    call @{}({})\n", symbol, args.join(", "))),
            }
        }
        Instruction::CallIndirect {
            out,
            table,
            slot,
            args,
        } => {
            let args: Vec<String> = args.iter().map(|a| value_name(*a)).collect();
            match out {
                Some(temp) => buf.push_str(&format!(
                    "    t{} = call_indirect {}[{}]({}) : {}\n",
                    temp.into_raw(),
                    value_name(*table),
                    slot,
                    args.join(", "),
                    function.temps[*temp].ty.dump()
                )),
                None => buf.push_str(&format!(
                    "    call_indirect {}[{}]({})\n",
                    value_name(*table),
                    slot,
                    args.join(", ")
                )),
            }
        }
        Instruction::Invoke {
            out,
            symbol,
            args,
            normal,
            unwind,
        } => {
            let args: Vec<String> = args.iter().map(|a| value_name(*a)).collect();
            buf.push_str(&format!(
                "    invoke @{}({}) normal @{} unwind @{}\n",
                symbol,
                args.join(", "),
                block_name(function, *normal),
                block_name(function, *unwind)
            ));
            let _ = out;
        }
        Instruction::InvokeIndirect {
            table,
            slot,
            args,
            normal,
            unwind,
            ..
        } => {
            let args: Vec<String> = args.iter().map(|a| value_name(*a)).collect();
            buf.push_str(&format!(
                "    invoke_indirect {}[{}]({}) normal @{} unwind @{}\n",
                value_name(*table),
                slot,
                args.join(", "),
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
            element_scan,
        } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            let scan = match element_scan {
                RefScan::None => String::new(),
                RefScan::References(offsets) if offsets == &[0] => String::new(),
                _ => format!(" scan={}", element_scan.dump()),
            };
            buf.push_str(&format!(
                "    t{} = array_alloc ({}){} : {}\n",
                out.into_raw(),
                elements.join(", "),
                scan,
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ArrayLen { out, operand } => buf.push_str(&format!(
            "    t{} = array_len {} : {}\n",
            out.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArrayGet { out, array, index } => buf.push_str(&format!(
            "    t{} = array_get {} {} : {}\n",
            out.into_raw(),
            value_name(*array),
            value_name(*index),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArraySet {
            array,
            index,
            value,
        } => buf.push_str(&format!(
            "    array_set {} {} {}\n",
            value_name(*array),
            value_name(*index),
            value_name(*value)
        )),
        Instruction::ArrayClone { out, operand } => buf.push_str(&format!(
            "    t{} = array_clone {} : {}\n",
            out.into_raw(),
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
