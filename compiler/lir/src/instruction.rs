use super::*;

#[derive(Debug)]
pub struct BasicBlock {
    pub name: String,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

/// Address-only operand for one outbound C-ABI argument.
///
/// The operand names the local that owns the argument's complete physical
/// storage. It cannot be forged from an arbitrary raw pointer: codegen binds
/// the local's exact [`LirType`] to the corresponding [`CType::storage_type`]
/// before passing its address to the generated C bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CArgumentStorage(LocalId);

impl CArgumentStorage {
    pub const fn address_of(local: LocalId) -> Self {
        Self(local)
    }

    pub const fn local(self) -> LocalId {
        self.0
    }
}

/// A value usable as an instruction operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Value {
    /// Contents of a local's stack slot (loaded implicitly).
    Local(LocalId),
    /// A function parameter (0-based).
    Param(u32),
    Temp(TempId),
    IntegerConst(LirIntegerConstant),
    MachineScalar(MachineScalarValue),
    BoolConst(bool),
    NullPointer(PointerKind),
    /// Address of a local or external TypeDescriptor.
    TypeDescriptor(TypeDescriptorRef),
    /// Address of one complete function-local recursive root scan program.
    RootScan(RootScanId),
    /// Address of a global constant.
    Global(GlobalId),
    /// Address of one codegen-emitted initialization-unit descriptor.
    InitializationUnit(InitializationUnitId),
    /// Address of exact typed local storage, consumable only as an outbound
    /// C-ABI bridge argument.
    CArgumentStorage(CArgumentStorage),
}

#[derive(Debug)]
pub enum Instruction {
    BoxValue {
        out: TempId,
        payload: BoxPayload,
        safepoint: SafepointSiteRef,
        live: StatepointLiveSet,
    },
    UnboxValue {
        object: Value,
        result: UnboxResult,
    },
    /// Equality over Boolean, raw pointer-shaped values, or one internal
    /// machine scalar domain. Source integer operations use the typed variants
    /// below and cannot enter this generic path.
    BinOp {
        out: TempId,
        op: BinOp,
        lhs: Value,
        rhs: Value,
    },
    /// Boolean negation. Source integer unary operations use `IntegerUnary`.
    UnaryOp {
        out: TempId,
        op: UnOp,
        operand: Value,
    },
    IntegerUnary {
        out: TempId,
        kind: IntegerKind,
        operation: IntegerUnaryOperation,
        operand: Value,
    },
    IntegerBinary {
        out: TempId,
        kind: IntegerKind,
        operation: IntegerBinaryOperation,
        lhs: Value,
        rhs: Value,
    },
    /// Division and remainder whose exceptional and signed-overflow cases
    /// have already been split in MIR. Codegen may emit the primitive LLVM
    /// operation directly and must not reconstruct those branches.
    SafeIntegerDivRem {
        out: TempId,
        kind: IntegerKind,
        operation: IntegerDivRemOperation,
        lhs: Value,
        rhs: Value,
    },
    IntegerCompare {
        out: TempId,
        kind: IntegerKind,
        comparison: IntegerComparison,
        lhs: Value,
        rhs: Value,
    },
    /// Three-way comparison always produces canonical source `Long` (`I64`).
    IntegerCompareTo {
        out: TempId,
        operand_kind: IntegerKind,
        lhs: Value,
        rhs: Value,
    },
    /// `normalized_count` is the source `Long` count after MIR has masked it
    /// and converted it to the operand's scalar width.
    IntegerShift {
        out: TempId,
        kind: IntegerKind,
        operation: IntegerShiftOperation,
        value: Value,
        normalized_count: Value,
    },
    IntegerConvert {
        out: TempId,
        source_kind: IntegerKind,
        target_kind: IntegerKind,
        operand: Value,
    },
    /// A logical value whose complete storage contract has zero payload.
    MakeZstValue {
        out: TempId,
        value: LogicalZstValue,
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
    /// Non-atomic load of one compiler-owned state word from managed storage.
    /// This is separate from `HeapLoad` so an `i64` field access cannot relabel
    /// a machine state, and one machine domain cannot be read as another.
    MachineHeapLoad {
        out: TempId,
        kind: MachineScalarKind,
        object: Value,
        offset: u64,
    },
    /// Acquire-load a 64-bit synthetic state word from managed storage.
    AtomicLoad {
        out: TempId,
        kind: MachineScalarKind,
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
        safepoint: SafepointSiteRef,
        roots: NativeSafeRootSet,
    },
    NativeGlobalStore {
        global: NativeGlobalId,
        value: Value,
        safepoint: SafepointSiteRef,
        roots: NativeSafeRootSet,
    },
    NativeGlobalAddress {
        out: TempId,
        global: NativeGlobalId,
        safepoint: SafepointSiteRef,
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
    /// Non-atomic initialization store of one compiler-owned state word into
    /// managed storage. Later concurrent accesses use the atomic variants.
    MachineHeapStore {
        kind: MachineScalarKind,
        object: Value,
        offset: u64,
        value: Value,
    },
    /// Release-store a 64-bit synthetic state word in managed storage.
    AtomicStore {
        kind: MachineScalarKind,
        object: Value,
        offset: u64,
        value: Value,
    },
    /// Acq_rel/acquire compare-exchange of a 64-bit synthetic state word.
    /// `out` receives the observed old word.
    AtomicCompareExchange {
        out: TempId,
        kind: MachineScalarKind,
        object: Value,
        offset: u64,
        expected: Value,
        replacement: Value,
    },
    /// Materialize the address of a module function as an opaque code
    /// pointer. It is metadata, not a managed reference.
    FunctionAddress {
        out: TempId,
        target: FunctionAddressTarget,
    },
    ForeignCallbackRegister {
        out: TempId,
        bridge: ForeignCallbackBridgeId,
        closure: Value,
    },
    ForeignCallbackOperation(ForeignCallbackOperation),
    ULongToPtr {
        out: TempId,
        value: Value,
    },
    PtrToULong {
        out: TempId,
        value: Value,
    },
    /// Read a payload whose complete storage contract is nonzero.
    RawLoad {
        out: TempId,
        pointer: Value,
        pointee: AbiValue,
    },
    /// Write a payload whose complete storage contract is nonzero.
    RawStore {
        pointer: Value,
        value: Value,
        pointee: AbiValue,
    },
    /// Pointer displacement by `element_offset * element_size`.  The offset
    /// remains either a source pointer index or the compiler-owned
    /// `PointerElementOffset` domain; the layout stride is a dedicated field
    /// and never becomes a source integer value.
    PtrOffset {
        out: TempId,
        pointer: Value,
        element_offset: Value,
        element_size: std::num::NonZeroU64,
        subtract: bool,
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
    /// End the innermost active catch (`scoop_rt_end_catch()`).
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
        safepoint: SafepointSiteRef,
        live: StatepointLiveSet,
    },
    /// Allocate one fresh array and fill it from already evaluated element
    /// values and source arrays. Source arrays are copied in order.
    ArrayAssembly {
        out: TempId,
        parts: Vec<ArrayAssemblyPart>,
        array_type: ArrayTypeId,
        safepoint: SafepointSiteRef,
        live: StatepointLiveSet,
    },
    /// `array.size` (canonical source `Long`, represented by `I64`).
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
    /// `Array(m)` / `MutableArray(a)` conversion with fresh reference identity.
    ArrayClone {
        out: TempId,
        operand: Value,
        /// Exact source array application, checked before reading its payload.
        source_type: ArrayTypeId,
        /// Target array application (`Array<T>` or `MutableArray<T>`).
        array_type: ArrayTypeId,
        safepoint: SafepointSiteRef,
        live: StatepointLiveSet,
    },
    /// Enum operations. The representation (niche pointer or tagged
    /// union) is fixed by `EnumDef::repr`, so codegen translates these
    /// mechanically.
    /// Construct a variant value.
    EnumWrap {
        out: TempId,
        variant: LirVariantRef,
        fields: Vec<Value>,
    },
    /// Read the variant tag (result `MachineScalar(EnumTag)`; niche: null
    /// test).
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
    /// Test whether `operand` contains one checked variant.  The result is
    /// canonical Boolean (`I1`); physical tag/null details remain private to
    /// the selected enum representation.
    VariantTest {
        out: TempId,
        operand: Value,
        variant: LirVariantRef,
    },
    /// Project one payload field after a matching [`Instruction::VariantTest`]
    /// true edge.  Module validation proves that edge dominates this use and
    /// derives the exact result type from `variant` plus `field`.
    VariantPayloadProject {
        out: TempId,
        operand: Value,
        field: LirVariantFieldRef,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionAddressTarget {
    Local(LocalFunctionRef),
    CallbackTrampoline(CallbackBridgeId),
}

impl Instruction {
    /// Function-local safepoint reference and the semantic role fixed by this
    /// instruction variant. NoGc instructions have no safepoint.
    pub fn safepoint(&self) -> Option<(SafepointSiteRole, SafepointSiteRef)> {
        match self {
            Self::NativeGlobalLoad { safepoint, .. }
            | Self::NativeGlobalStore { safepoint, .. }
            | Self::NativeGlobalAddress { safepoint, .. } => {
                Some((SafepointSiteRole::NativeSafeTransition, *safepoint))
            }
            Self::Call { site } => match site {
                CallSite::Managed(site) => Some((SafepointSiteRole::ManagedCall, site.safepoint)),
                CallSite::NativeSafe(site) => {
                    Some((SafepointSiteRole::NativeSafeTransition, site.safepoint))
                }
                CallSite::NativeBorrowed(site) => {
                    Some((SafepointSiteRole::NativeBorrowedTransition, site.safepoint))
                }
                CallSite::NoGc(_) => None,
            },
            Self::ManagedPoll { site } => Some((SafepointSiteRole::ManagedPoll, site.safepoint)),
            Self::Invoke {
                site: InvokeSite::Managed(site),
            } => Some((SafepointSiteRole::ManagedInvoke, site.safepoint)),
            Self::ArrayAlloc { safepoint, .. }
            | Self::ArrayAssembly { safepoint, .. }
            | Self::ArrayClone { safepoint, .. }
            | Self::BoxValue { safepoint, .. } => {
                Some((SafepointSiteRole::ManagedCall, *safepoint))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayAssemblyPart {
    Element(Value),
    CopyArray(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Eq,
    Ne,
    /// Equality in one internal scalar domain.  The kind is part of the
    /// opcode rather than inferred from the current `i64` representation.
    MachineEq(MachineScalarKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerUnaryOperation {
    Plus,
    Negate,
    BitwiseNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerBinaryOperation {
    Add,
    Subtract,
    Multiply,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerDivRemOperation {
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerComparison {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerShiftOperation {
    Left,
    ArithmeticRight,
    LogicalRight,
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
