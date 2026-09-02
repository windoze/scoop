use super::*;

#[derive(Debug)]
pub struct Function {
    /// Whether this body participates in managed GC instrumentation.
    pub gc_effect: GcEffect,
    pub name: String,
    /// Mangled symbol; `scoop.<name>`, `scoop.<name>$<args>` for
    /// monomorphized instances, or `scoop_main` for the entry.
    pub symbol: String,
    pub params: Vec<Param>,
    pub return_ty: Type,
    pub body: Body,
}

#[derive(Debug)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub abi: ExternAbi,
    pub calling_convention: CallingConvention,
    pub gc_effect: GcEffect,
    pub params: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
    Scoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    /// Parameters are (immutable) locals.
    pub local: LocalId,
}

#[derive(Debug)]
pub struct Body {
    pub locals: Arena<Local>,
    pub blocks: Arena<BasicBlock>,
    pub entry: BlockId,
}

impl Body {
    /// A valid body for signature-only function shells. It contains one
    /// unreachable entry block so downstream code never handles a missing
    /// entry or an empty block arena.
    pub fn unreachable(locals: Arena<Local>) -> Self {
        let mut blocks = Arena::new();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
            unwind: None,
        });
        Self {
            locals,
            blocks,
            entry,
        }
    }
}

#[derive(Debug)]
pub struct BasicBlock {
    pub name: String,
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
    /// Explicit exceptional successor for every potentially throwing user,
    /// virtual, or interface call evaluated in this block.
    pub unwind: Option<BlockId>,
}

#[derive(Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum StatementKind {
    Expr(Expr),
    Call(CallEffect),
    ValDecl {
        local: LocalId,
        init: Expr,
    },
    Assign {
        local: LocalId,
        value: Expr,
    },
    GlobalAssign {
        global: GlobalId,
        value: Expr,
    },
    ArraySet {
        array_type: ClassId,
        array: Expr,
        index: Expr,
        value: Expr,
    },
    FieldSet {
        object: Expr,
        index: u32,
        value: Expr,
    },
    /// Release-store an aligned 64-bit synthetic state field. This is a
    /// distinct MIR operation so coroutine synchronization cannot be lost by
    /// reconstructing atomic intent from field names downstream.
    AtomicFieldStore {
        object: Expr,
        index: u32,
        value: Expr,
    },
    Eh(EhStatement),
}

#[derive(Debug)]
pub enum CallEffect {
    /// A call whose source result type is `Unit`.
    Unit(Call),
    /// A value-producing call. The destination local carries the complete,
    /// non-optional result type.
    Value { destination: LocalId, call: Call },
}

#[derive(Debug)]
pub enum EhStatement {
    /// Capture the active native exception into function-local EH slots.
    LandingPad {
        cleanup: bool,
    },
    /// Begin the catch represented by the captured exception.
    BeginCatch,
    EndCatch,
}

#[derive(Debug)]
pub enum Terminator {
    Goto(BlockId),
    Branch {
        cond: Expr,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return {
        value: Option<Expr>,
    },
    /// Throw a managed exception. `unwind` is the same edge recorded on
    /// the owning block and is repeated here because this terminator itself
    /// initiates unwinding rather than containing a call expression.
    Throw {
        exception: Expr,
        unwind: Option<BlockId>,
    },
    /// Rethrow the currently active native catch.
    Rethrow {
        unwind: Option<BlockId>,
    },
    /// Continue native unwinding with the exception record captured by the
    /// nearest landing/cleanup pad.
    Resume,
    Trap {
        message: StringConstId,
    },
    Unreachable,
}

/// A MIR expression whose semantic result type is complete by construction.
/// Consumers must use `ty` directly; reconstructing it from the expression
/// shape, surrounding local or expected context is forbidden.
#[derive(Debug, Clone)]
pub struct Expr {
    pub ty: Type,
    pub kind: ExprKind,
}

impl Expr {
    pub fn new(ty: Type, kind: ExprKind) -> Self {
        Self { ty, kind }
    }

    pub fn local(local: LocalId, ty: Type) -> Self {
        Self::new(ty, ExprKind::Local(local))
    }

    pub fn int(value: i64) -> Self {
        Self::new(Type::Int, ExprKind::IntLiteral(value))
    }

    pub fn bool(value: bool) -> Self {
        Self::new(Type::Boolean, ExprKind::BoolLiteral(value))
    }

    pub fn unit() -> Self {
        Self::new(Type::Unit, ExprKind::UnitLiteral)
    }

    pub fn caught_exception() -> Self {
        Self::new(Type::Any, ExprKind::CaughtException)
    }

    pub fn enum_tag(operand: Expr) -> Self {
        Self::new(Type::Int, ExprKind::EnumTag(Box::new(operand)))
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    StringConst(StringConstId),
    IntLiteral(i64),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: StructId,
        args: Vec<Expr>,
    },
    /// Class instantiation; mir-lower generates a constructor
    /// function per class and this becomes a plain call to it.
    ClassInit {
        class_id: ClassId,
        args: Vec<Expr>,
    },
    ClosureAlloc {
        class: ClosureClassId,
        captures: Vec<Expr>,
    },
    /// Read one inline capture field from a concrete closure object.
    ClosureCapture {
        closure: Box<Expr>,
        class: ClosureClassId,
        index: u32,
    },
    Local(LocalId),
    GlobalRead(GlobalId),
    PtrFromUInt {
        operand: Box<Expr>,
        pointee: Box<Type>,
    },
    PtrToUInt(Box<Expr>),
    PtrCast {
        operand: Box<Expr>,
        pointee: Box<Type>,
    },
    PtrLoad {
        pointer: Box<Expr>,
        pointee: Box<Type>,
        offset: Option<Box<Expr>>,
    },
    PtrStore {
        pointer: Box<Expr>,
        pointee: Box<Type>,
        offset: Option<Box<Expr>>,
        value: Box<Expr>,
    },
    PtrOffset {
        pointer: Box<Expr>,
        pointee: Box<Type>,
        offset: Box<Expr>,
        subtract: bool,
    },
    AddressOf {
        local: LocalId,
        pointee: Box<Type>,
    },
    GlobalAddress {
        global: GlobalId,
        pointee: Box<Type>,
    },
    SizeOf(Box<Type>),
    AlignOf(Box<Type>),
    FunPtrNull(FunctionTypeId),
    FunctionAddress {
        callback: CallbackBridgeId,
    },
    ForeignCallbackRegister {
        bridge: ForeignCallbackBridgeId,
        closure: Box<Expr>,
    },
    ForeignCallbackOperation {
        operation: ForeignCallbackOperation,
        callback: Box<Expr>,
    },
    /// The managed exception pointer produced by the active `BeginCatch`.
    /// It is only valid in blocks dominated by that statement.
    CaughtException,
    /// Zero-cost reference retyping (for example an `Any` local narrowed by
    /// a class smart cast). The explicit result type keeps downstream field
    /// and dispatch reconstruction independent of the local's declared type.
    Retype {
        operand: Box<Expr>,
        ty: Box<Type>,
    },
    /// Field or element access; `index` is 0-based for both structs
    /// and tuples.
    FieldAccess {
        receiver: Box<Expr>,
        index: u32,
    },
    /// Acquire-load an aligned 64-bit synthetic state field.
    AtomicFieldLoad {
        object: Box<Expr>,
        index: u32,
    },
    /// Compare-exchange an aligned 64-bit synthetic state field. The returned
    /// value is the observed old word; success is acq_rel and failure acquire.
    AtomicFieldCompareExchange {
        object: Box<Expr>,
        index: u32,
        expected: Box<Expr>,
        replacement: Box<Expr>,
    },
    /// Box a value type into `Any` / an interface (spec 4.4.4).
    Box(Box<Expr>),
    /// Unbox a reference back to a value type.
    Unbox(Box<Expr>),
    /// `expr is T` (result `Int`-as-bool). The checked type is in
    /// `check_ty`.
    IsInstance {
        operand: Box<Expr>,
        check_ty: Box<Type>,
    },
    /// `as` (traps on failure) or `as?` (`optional`, result
    /// `Option<T>`); the target type comes from context.
    Cast {
        operand: Box<Expr>,
        optional: bool,
    },
    /// `[e1, ...]`; `array_type` is the exact fully specialized intrinsic
    /// class application selected by HIR.
    ArrayLiteral {
        array_type: ClassId,
        elements: Vec<Expr>,
    },
    /// Subscript read; result is the element type.
    ArrayGet {
        array_type: ClassId,
        array: Box<Expr>,
        index: Box<Expr>,
    },
    /// `array.size`; result is `Int`.
    ArrayLen {
        array_type: ClassId,
        operand: Box<Expr>,
    },
    /// Array-kind conversion (constructor or method form): memcpy snapshot.
    /// Both source and target identities are explicit and complete.
    ArrayClone {
        source_type: ClassId,
        target_type: ClassId,
        operand: Box<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    /// Variant construction. The enclosing `Expr::ty` is the instantiated
    /// enum type; `fields` are the variant's values in declaration order.
    VariantConstruct {
        variant: u32,
        fields: Vec<Expr>,
    },
    /// Read the variant tag of an enum value (Int).
    EnumTag(Box<Expr>),
    /// Read field `index` of variant `variant` from an enum value.
    /// Only evaluated on a path where the tag is known to match.
    EnumField {
        operand: Box<Expr>,
        variant: u32,
        index: u32,
    },
}

#[derive(Debug, Clone)]
pub struct Call {
    pub target: CallTarget,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct CallTarget {
    pub kind: CallKind,
    /// Fully resolved callee.
    pub callee: Callee,
}

#[derive(Debug, Clone)]
pub enum CallKind {
    Direct,
    /// vtable slot (load `td` from the receiver, load `vtable[slot]`).
    Virtual {
        slot: u32,
    },
    /// itable lookup (`scoop_rt_itable_lookup(td, iface_td)`), then
    /// `slot` within the returned table.
    Interface {
        interface: InterfaceId,
        slot: u32,
    },
    /// Managed function-value invocation. Argument 0 is the closure ref;
    /// LIR loads its code pointer and calls it with the exact signature.
    Closure {
        function_type: FunctionTypeId,
    },
    /// Runtime-selected function-type bridge from the source closure's
    /// TypeDescriptor table. Argument 0 is the source closure reference.
    FunctionBridge {
        function_type: FunctionTypeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// A user function defined in this Cone.
    User(FunctionId),
    /// A monomorphized generic function defined in this Cone.
    Monomorphized(MonomorphizedFunctionId),
    /// A bodyless native declaration in the independent extern arena.
    Extern(ExternFunctionId),
    /// Typed marker used only between CFG construction and the coroutine
    /// state-machine pass. The final MIR handed to LIR contains no such
    /// callee; `register` identifies the concrete protocol method shell.
    CoroutineSuspend { register: MonomorphizedFunctionId },
    /// There is no statically selected function; the exact signature is the
    /// complete typed call target carried through CFG construction.
    Closure(FunctionTypeId),
    /// No source signature is statically known (an `Any`/interface cast).
    /// The target signature selects one bridge-table entry at runtime.
    FunctionBridge(FunctionTypeId),
    /// A runtime function (see `RuntimeFn::symbol`).
    Runtime(RuntimeFn),
}

/// Runtime functions called directly by compiler-generated operations.
/// Source-level core capabilities use ordinary declarations and extern calls;
/// they do not acquire entries in this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFn {
    /// Box one value; LIR supplies the addressable payload and its complete
    /// recursive scan program to the managed runtime entry.
    Box,
    /// `scoop_rt_is_instance(obj, td)`
    IsInstance,
    /// `scoop_rt_itable_lookup(td, iface_td)`
    ITableLookup,
    /// GC facilities (spec 14.1; M9 via intrinsics, see
    /// docs/milestone9/DESIGN.md 5.2).
    Pin,
    Unpin,
    GetHandle,
    ReleaseHandle,
    /// Test-only GC hooks (not in the spec, milestone9 DESIGN 5.2).
    GcCollect,
    GcStats,
    /// ABI exception buffer -> ordinary managed object (runtime spec 5).
    MaterializeException,
    StringConcat,
    /// Noreturn runtime trap, called with a message string constant
    /// (M4: `!!` on `None`; M8: real exceptions).
    Trap,
}

impl RuntimeFn {
    pub fn symbol(self) -> &'static str {
        match self {
            RuntimeFn::Box => "scoop_rt_box",
            RuntimeFn::IsInstance => "scoop_rt_is_instance",
            RuntimeFn::ITableLookup => "scoop_rt_itable_lookup",
            RuntimeFn::Pin => "scoop_rt_pin",
            RuntimeFn::Unpin => "scoop_rt_unpin",
            RuntimeFn::GetHandle => "scoop_rt_get_handle",
            RuntimeFn::ReleaseHandle => "scoop_rt_release_handle",
            RuntimeFn::GcCollect => "scoop_rt_gc_collect",
            RuntimeFn::GcStats => "scoop_rt_gc_stats",
            RuntimeFn::MaterializeException => "scoop_rt_materialize_exception",
            RuntimeFn::StringConcat => "scoop_rt_string_concat",
            RuntimeFn::Trap => "scoop_rt_trap",
        }
    }
}

/// Primitive operations only: aggregate equality has been expanded by
/// mir-lower (docs/milestone2/DESIGN.md 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    IntAdd,
    IntSub,
    IntMul,
    IntDiv,
    IntLt,
    IntLe,
    IntGt,
    IntGe,
    IntEq,
    IntNe,
    BoolEq,
    BoolNe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    IntNeg,
    BoolNot,
}
