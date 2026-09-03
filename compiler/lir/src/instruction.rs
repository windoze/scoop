use super::*;

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
    /// Allocate one fresh array and fill it from already evaluated element
    /// values and source arrays. Source arrays are copied in order.
    ArrayAssembly {
        out: TempId,
        parts: Vec<ArrayAssemblyPart>,
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
pub enum ArrayAssemblyPart {
    Element(Value),
    CopyArray(Value),
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
