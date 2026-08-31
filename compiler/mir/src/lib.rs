//! MIR definitions and MIR meta: the data channel between MIR and LIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone2/DESIGN.md` section 2.3.
//!
//! Structural equality on aggregates is already expanded by mir-lower
//! into primitive comparisons and runtime calls, so MIR `BinOp` only
//! contains primitive operations. Since M10 every emitted function body
//! is a CFG and calls are explicit effect statements; [`Expr`] cannot
//! contain a call.

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub type FunctionId = Idx<Function>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type GlobalId = Idx<Global>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type ClosureClassId = Idx<ClosureClass>;
pub type ClosureInvokeFunctionId = Idx<ClosureInvokeFunction>;
pub type ClosureAdapterId = Idx<ClosureAdapter>;
pub type DynamicClosureAdapterId = Idx<DynamicClosureAdapter>;
pub type MonomorphizedFunctionId = Idx<MonomorphizedFunction>;
pub type StringConstId = Idx<StringConst>;
pub type StructId = Idx<StructDef>;
pub type EnumId = Idx<EnumDef>;
pub type ClassId = Idx<ClassDef>;
pub type InterfaceId = Idx<InterfaceDef>;
pub type LocalId = Idx<Local>;
pub type BlockId = Idx<BasicBlock>;
pub type CoroutineFunctionId = Idx<CoroutineFunction>;
pub type CoroutineStepId = Idx<CoroutineStep>;
pub type CoroutineSlotId = Idx<CoroutineSlot>;
pub type CoroutineFrameId = Idx<CoroutineFrame>;
pub type CoroutineResumePointId = Idx<CoroutineResumePoint>;

/// Mangled symbol of the program entry point (called by the C runtime).
pub const ENTRY_SYMBOL: &str = "scoop_main";

/// Mangle a user function name (entry point maps to `ENTRY_SYMBOL`).
pub fn mangle_function(name: &str, is_entry: bool) -> String {
    if is_entry {
        ENTRY_SYMBOL.to_string()
    } else {
        format!("scoop.{name}")
    }
}

pub fn mangle_global(name: &str) -> String {
    format!("scoop.global.{name}")
}

/// Mangle a monomorphized instance: `scoop.<name>$<encoded type args>`.
pub fn mangle_instance(module: &Module, name: &str, type_args: &[Type]) -> String {
    let args: Vec<String> = type_args.iter().map(|t| encode_type(module, t)).collect();
    format!("scoop.{name}${}", args.join("_"))
}

/// Mangle a monomorphized instance when several generic definitions
/// share the same qualified name. The generic-definition discriminator
/// is local to the Cone and only appears for such overload groups, so
/// the ordinary compact instance symbol remains unchanged.
pub fn mangle_generic_overload(
    module: &Module,
    name: &str,
    type_args: &[Type],
    generic_discriminator: u32,
) -> String {
    format!(
        "{}.g{generic_discriminator}",
        mangle_instance(module, name, type_args)
    )
}

/// Mangle one overload of a name shared by several functions (M7):
/// `scoop.<name>.<encoded params>` — `scoop.show.I`,
/// `scoop.println.S`; a zero-parameter overload gets an empty encoding
/// (`scoop.f.`). `.` introduces the overload encoding while `$` stays
/// reserved for monomorphized instances, so the two never collide.
pub fn mangle_overload(module: &Module, name: &str, params: &[Type]) -> String {
    format!("scoop.{name}.{}", encode_params(module, params))
}

/// Add the hidden coroutine-ABI discriminator to an already mangled source
/// callable. `$suspend` cannot collide with a source identifier or the `$`
/// type-argument encoding of a monomorphized function.
pub fn mangle_suspend(symbol: &str) -> String {
    format!("{symbol}$suspend")
}

/// The `_`-joined parameter encoding shared by overload mangling and
/// dispatch signature keys.
pub fn encode_params(module: &Module, params: &[Type]) -> String {
    params
        .iter()
        .map(|t| encode_type(module, t))
        .collect::<Vec<_>>()
        .join("_")
}

/// Compact type encoding for mangling (e.g. `scoop.identity$I`).
pub fn encode_type(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "U".to_string(),
        Type::Int => "I".to_string(),
        Type::UInt => "V".to_string(),
        Type::Boolean => "B".to_string(),
        Type::String => "S".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Class(id) => module.classes[*id].name.clone(),
        Type::Interface(id) => module.interfaces[*id].name.clone(),
        Type::Any => "Any".to_string(),
        Type::Array(inner) => format!("A{}X", encode_type(module, inner)),
        Type::MutableArray(inner) => format!("M{}X", encode_type(module, inner)),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| encode_type(module, t)).collect();
            format!("T{}X", inner.join("_"))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let kind = if function.is_suspend { "S" } else { "F" };
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| encode_type(module, ty))
                .collect::<Vec<_>>()
                .join("_");
            format!(
                "{kind}{parameters}R{}X",
                encode_type(module, &function.return_type)
            )
        }
        Type::Ptr(inner) => format!("P{}X", encode_type(module, inner)),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| encode_type(module, ty))
                .collect::<Vec<_>>()
                .join("_");
            format!(
                "N{parameters}R{}X",
                encode_type(module, &function.return_type)
            )
        }
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            if args.is_empty() {
                format!("E{name}")
            } else {
                let inner: Vec<String> = args.iter().map(|t| encode_type(module, t)).collect();
                format!("E{}_{}X", name, inner.join("_"))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Int,
    /// Unsigned 64-bit integer (`UInt`, spec 11.2; same machine word
    /// as `Int`, mapped to `i64` at LIR).
    UInt,
    Boolean,
    String,
    Struct(StructId),
    /// A reference type declared with `class`.
    Class(ClassId),
    /// An interface type (dispatch through itables, impl spec 2.9).
    Interface(InterfaceId),
    /// The root of all types; boxed value types live behind it.
    Any,
    /// Built-in array types (M5, see hir::Type). Invariant (spec 10.4).
    Array(Box<Type>),
    MutableArray(Box<Type>),
    Tuple(Vec<Type>),
    /// Concrete managed function signature. Function values have reference
    /// representation; closure classes are materialized by M11 conversion.
    Function(FunctionTypeId),
    /// Typed raw data pointer; representation is one native pointer word.
    Ptr(Box<Type>),
    /// Typed C function pointer; identity includes its exact signature.
    FunPtr(FunctionTypeId),
    /// An instantiated enum type (including `Option<T>` since M4).
    Enum(EnumId, Vec<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    pub is_suspend: bool,
    pub parameter_types: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug)]
pub struct ClosureClass {
    pub name: String,
    pub function_type: FunctionTypeId,
    pub invoke: ClosureInvokeFunctionId,
    pub captures: Vec<Field>,
    /// Function-type views supported by this exact closure class. Each slot
    /// is a typed forwarding entry whose ABI is `target`.
    pub bridges: Vec<FunctionBridge>,
}

#[derive(Debug)]
pub struct FunctionBridge {
    pub target: FunctionTypeId,
    pub function: FunctionId,
}

#[derive(Debug)]
pub struct ClosureInvokeFunction {
    pub function: FunctionId,
}

/// Typed identity reserved for variance bridges. M11's variance gate fills
/// this arena; keeping it distinct now prevents adapters from being confused
/// with source closure classes.
#[derive(Debug)]
pub struct ClosureAdapter {
    pub class: ClosureClassId,
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}

/// Adapter used after a runtime `Any`/interface-to-function check. Its source
/// signature is discovered from the captured closure's TypeDescriptor bridge
/// table, while its exposed invoke ABI is exactly `target`.
#[derive(Debug)]
pub struct DynamicClosureAdapter {
    pub class: ClosureClassId,
    pub target: FunctionTypeId,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    pub c_layout: Option<CLayout>,
    pub interior_mutable: bool,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CLayout {
    pub aligned: u8,
    pub packed: u8,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// An instantiated enum definition (M4): variants with concrete field
/// types. `name` is the mangled instance name (e.g. `Option$I`).
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug)]
pub struct VariantDef {
    pub name: String,
    /// Fields in declaration order (named and positional forms both
    /// normalized; positional fields carry `_1`-style names).
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// A class definition with its dispatch layout fixed by mir-lower
/// (impl spec 2.9).
#[derive(Debug)]
pub struct ClassDef {
    pub modifier: ClassModifier,
    pub name: String,
    /// Constructor properties in declaration order.
    pub fields: Vec<Field>,
    pub base_class: Option<ClassId>,
    pub interfaces: Vec<InterfaceId>,
    /// vtable slots: 0..2 are the `Any` defaults
    /// (`RuntimeFn::AnyEquals/AnyHashCode/AnyToString`), then user
    /// methods in vtable order (overrides share the base slot).
    pub vtable: Vec<TableSlot>,
    /// itable entries, one per implemented interface (pointer-keyed
    /// lookup at runtime).
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug)]
pub enum TableSlot {
    Function(FunctionId),
    Runtime(RuntimeFn),
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface: InterfaceId,
    pub slots: Vec<TableSlot>,
}

#[derive(Debug)]
pub struct InterfaceDef {
    pub name: String,
    /// Method names in declaration order (itable slot indices).
    pub methods: Vec<String>,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[derive(Debug)]
pub struct Module {
    pub functions: Arena<Function>,
    pub extern_functions: Arena<ExternFunction>,
    pub globals: Arena<Global>,
    pub function_types: Arena<FunctionType>,
    pub closure_classes: Arena<ClosureClass>,
    pub closure_invoke_functions: Arena<ClosureInvokeFunction>,
    /// User functions in declaration order (builtins have no MIR body).
    pub top_level: Vec<FunctionId>,
    pub strings: Arena<StringConst>,
    pub structs: Arena<StructDef>,
    pub enums: Arena<EnumDef>,
    pub classes: Arena<ClassDef>,
    pub interfaces: Arena<InterfaceDef>,
    pub entry: FunctionId,
    pub meta: MirMeta,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    pub symbol: String,
    pub ty: Type,
    pub mutable: bool,
    pub storage: GlobalStorage,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Local {
        thread_local: bool,
        initializer: ConstantValue,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i64),
    Bool(bool),
    NullPtr,
    NullFunPtr,
    Struct {
        struct_id: StructId,
        fields: Vec<ConstantValue>,
    },
}

/// Per-Cone MIR metadata (impl spec 2.3).
#[derive(Debug, Default)]
pub struct MirMeta {
    pub dispatch_tables: Vec<DispatchTable>,
    /// Monomorphized function instances in creation order. The entry
    /// records the emitted symbol and its generic HIR source without
    /// leaking HIR ids across the stage boundary.
    pub instances: Arena<MonomorphizedFunction>,
    /// Concrete hidden-ABI suspend callables, indexed independently from the
    /// ordinary function arena.
    pub coroutine_functions: Arena<CoroutineFunction>,
    /// Concrete `CoroutineStep<R>` internal enums, deduplicated by `R`.
    pub coroutine_steps: Arena<CoroutineStep>,
    /// Tagged frame slots, deduplicated by their carried value type.
    pub coroutine_slots: Arena<CoroutineSlot>,
    /// Heap frame generated for each suspend callable that can really suspend.
    pub coroutine_frames: Arena<CoroutineFrame>,
    /// Per-call-site continuation adapters and their typed resume state.
    pub coroutine_resume_points: Arena<CoroutineResumePoint>,
    pub closure_adapters: Arena<ClosureAdapter>,
    pub dynamic_closure_adapters: Arena<DynamicClosureAdapter>,
}

#[derive(Debug)]
pub struct CoroutineFunction {
    pub function: FunctionId,
    pub source_return: Type,
    pub step: CoroutineStepId,
    pub lowering: CoroutineLowering,
}

#[derive(Debug)]
pub enum CoroutineLowering {
    Immediate,
    StateMachine {
        frame: CoroutineFrameId,
        driver: FunctionId,
        resume_points: Vec<CoroutineResumePointId>,
    },
}

#[derive(Debug)]
pub struct CoroutineStep {
    pub enum_id: EnumId,
    pub result: Type,
}

#[derive(Debug)]
pub struct CoroutineSlot {
    pub enum_id: EnumId,
    pub value: Type,
}

#[derive(Debug)]
pub struct CoroutineFrame {
    pub class: ClassId,
    pub owner: CoroutineFunctionId,
}

#[derive(Debug)]
pub struct CoroutineResumePoint {
    pub frame: CoroutineFrameId,
    pub state: u32,
    pub result: Type,
    pub adapter: ClassId,
    pub resume: FunctionId,
    pub resume_with_exception: FunctionId,
}

/// Provenance of one concrete generic function emitted into the MIR
/// function arena. Its typed id is also what MIR call sites carry.
#[derive(Debug)]
pub struct MonomorphizedFunction {
    pub function: FunctionId,
    pub symbol: String,
    pub source: String,
    pub type_args: Vec<Type>,
}

#[derive(Debug)]
pub struct DispatchTable {
    pub owner: FunctionId,
    pub entries: Vec<FunctionId>,
}

#[derive(Debug)]
pub struct StringConst {
    pub value: String,
    /// Mangled global symbol, e.g. `scoop.str.0`.
    pub symbol: String,
}

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
        array: Expr,
        index: Expr,
        value: Expr,
    },
    FieldSet {
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

#[derive(Debug, Clone)]
pub enum Expr {
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
        function: FunctionId,
        signature: FunctionTypeId,
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
    /// `[e1, ...]` (the kind, Array vs MutableArray, is fixed by the
    /// producing context — LIR types record it).
    ArrayLiteral(Vec<Expr>),
    /// Subscript read; result is the element type.
    ArrayGet {
        array: Box<Expr>,
        index: Box<Expr>,
    },
    /// `array.size`; result is `Int`.
    ArrayLen(Box<Expr>),
    /// Array-kind conversion (constructor or method form): memcpy snapshot.
    ArrayClone(Box<Expr>),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    /// Variant construction; `ty` is the instantiated enum type.
    /// `fields` are the variant's field values in declaration order.
    VariantConstruct {
        ty: Type,
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

/// Runtime functions callable from generated code. The output shims
/// are temporary until M11 (docs/milestone1/DESIGN.md 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFn {
    /// `scoop_rt_box(td, payload, size)`
    Box,
    /// `scoop_rt_is_instance(obj, td)`
    IsInstance,
    /// `scoop_rt_itable_lookup(td, iface_td)`
    ITableLookup,
    /// The `Any` vtable defaults (slots 0..2).
    AnyEquals,
    AnyHashCode,
    AnyToString,
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
    /// Primitive output intrinsics backing core's `print`/`println`
    /// overloads (M7, docs/milestone7/DESIGN.md section 2).
    Write,
    IntToString,
    BoolToString,
    StringConcat,
    StringEq,
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
            RuntimeFn::AnyEquals => "scoop_rt_any_equals",
            RuntimeFn::AnyHashCode => "scoop_rt_any_hashcode",
            RuntimeFn::AnyToString => "scoop_rt_any_tostring",
            RuntimeFn::Pin => "scoop_rt_pin",
            RuntimeFn::Unpin => "scoop_rt_unpin",
            RuntimeFn::GetHandle => "scoop_rt_get_handle",
            RuntimeFn::ReleaseHandle => "scoop_rt_release_handle",
            RuntimeFn::GcCollect => "scoop_rt_gc_collect",
            RuntimeFn::GcStats => "scoop_rt_gc_stats",
            RuntimeFn::MaterializeException => "scoop_rt_materialize_exception",
            RuntimeFn::Write => "scoop_rt_print",
            RuntimeFn::IntToString => "scoop_rt_int_to_string",
            RuntimeFn::BoolToString => "scoop_rt_bool_to_string",
            RuntimeFn::StringConcat => "scoop_rt_string_concat",
            RuntimeFn::StringEq => "scoop_rt_string_eq",
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

/// Indented text dump for golden tests (`scoopc build --emit=mir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            GlobalStorage::Local {
                thread_local: false,
                ..
            } => "global".to_string(),
            GlobalStorage::Local {
                thread_local: true, ..
            } => "thread_local".to_string(),
            GlobalStorage::Extern {
                native_symbol,
                thread_local,
                ..
            } => format!(
                "extern {native_symbol}{}",
                if *thread_local { " thread_local" } else { "" }
            ),
        };
        out.push_str(&format!(
            "  global{} @{} {}: {} {storage}\n",
            id.into_raw().into_u32(),
            global.symbol,
            global.name,
            type_name(module, &global.ty)
        ));
    }
    for (id, extern_) in module.extern_functions.iter() {
        let abi = match extern_.abi {
            ExternAbi::C => "c",
            ExternAbi::Scoop => "scoop",
        };
        let params = extern_
            .params
            .iter()
            .map(|ty| type_name(module, ty))
            .collect::<Vec<_>>()
            .join(", ");
        let library = if extern_.library.is_empty() {
            String::new()
        } else {
            format!(" lib={}", extern_.library)
        };
        out.push_str(&format!(
            "  extern ef{} {} @{}({}) -> {} <abi={abi}{}{}>\n",
            id.into_raw().into_u32(),
            extern_.source_name,
            extern_.native_symbol,
            params,
            type_name(module, &extern_.return_type),
            if extern_.gc_effect == GcEffect::NoGc {
                " no-gc"
            } else {
                " managed"
            },
            library
        ));
    }
    for (_, def) in module.structs.iter() {
        let fields: Vec<String> = def
            .fields
            .iter()
            .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
            .collect();
        let mut attributes = Vec::new();
        if let Some(layout) = def.c_layout {
            attributes.push(format!(
                "c-layout aligned={} packed={}",
                layout.aligned, layout.packed
            ));
        }
        if def.interior_mutable {
            attributes.push("interior-mutable".to_string());
        }
        let attributes = if attributes.is_empty() {
            String::new()
        } else {
            format!(" <{}>", attributes.join(" "))
        };
        out.push_str(&format!(
            "  struct {} ({}){}\n",
            def.name,
            fields.join(", "),
            attributes
        ));
    }
    for (_, def) in module.enums.iter() {
        out.push_str(&format!("  enum {}\n", def.name));
        for variant in &def.variants {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
                .collect();
            out.push_str(&format!("    {}({})\n", variant.name, fields.join(", ")));
        }
    }
    for (_, def) in module.classes.iter() {
        out.push_str(&format!(
            "  class {} vtable={} itables={}\n",
            def.name,
            def.vtable.len(),
            def.itables.len()
        ));
    }
    for (_, def) in module.interfaces.iter() {
        out.push_str(&format!("  interface {}\n", def.name));
    }
    for (id, def) in module.closure_classes.iter() {
        let invoke = module.closure_invoke_functions[def.invoke].function;
        out.push_str(&format!(
            "  closure cc{} {} type=function_type{} invoke=@{} captures={}\n",
            id.into_raw().into_u32(),
            def.name,
            def.function_type.into_raw().into_u32(),
            module.functions[invoke].symbol,
            def.captures.len()
        ));
        for bridge in &def.bridges {
            out.push_str(&format!(
                "    bridge function_type{} -> @{}\n",
                bridge.target.into_raw().into_u32(),
                module.functions[bridge.function].symbol
            ));
        }
    }
    for (id, adapter) in module.meta.closure_adapters.iter() {
        out.push_str(&format!(
            "  adapter ca{} class=cc{} source=function_type{} target=function_type{}\n",
            id.into_raw().into_u32(),
            adapter.class.into_raw().into_u32(),
            adapter.source.into_raw().into_u32(),
            adapter.target.into_raw().into_u32()
        ));
    }
    for (id, adapter) in module.meta.dynamic_closure_adapters.iter() {
        out.push_str(&format!(
            "  dynamic_adapter da{} class=cc{} target=function_type{}\n",
            id.into_raw().into_u32(),
            adapter.class.into_raw().into_u32(),
            adapter.target.into_raw().into_u32()
        ));
    }
    for &id in &module.top_level {
        let function = &module.functions[id];
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_name(module, &p.ty)))
            .collect();
        out.push_str(&format!(
            "  fun {} @{}({}) -> {}{}\n",
            function.name,
            function.symbol,
            params.join(", "),
            type_name(module, &function.return_ty),
            if function.gc_effect == GcEffect::NoGc {
                " <no-gc>"
            } else {
                ""
            }
        ));
        for (block_id, block) in function.body.blocks.iter() {
            let unwind = block
                .unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!(
                "    bb{} {}{}\n",
                block_number(block_id),
                block.name,
                unwind
            ));
            dump_statements(
                module,
                &function.body.locals,
                &block.statements,
                3,
                &mut out,
            );
            dump_terminator(
                module,
                &function.body.locals,
                &block.terminator,
                3,
                &mut out,
            );
        }
    }
    for (id, step) in module.meta.coroutine_steps.iter() {
        out.push_str(&format!(
            "  coroutine_step cs{} {} result={}\n",
            id.into_raw().into_u32(),
            module.enums[step.enum_id].name,
            type_name(module, &step.result)
        ));
    }
    for (id, slot) in module.meta.coroutine_slots.iter() {
        out.push_str(&format!(
            "  coroutine_slot cl{} {} value={}\n",
            id.into_raw().into_u32(),
            module.enums[slot.enum_id].name,
            type_name(module, &slot.value)
        ));
    }
    for (id, frame) in module.meta.coroutine_frames.iter() {
        out.push_str(&format!(
            "  coroutine_frame cr{} {} owner=cf{}\n",
            id.into_raw().into_u32(),
            module.classes[frame.class].name,
            frame.owner.into_raw().into_u32()
        ));
    }
    for (id, point) in module.meta.coroutine_resume_points.iter() {
        out.push_str(&format!(
            "  coroutine_resume cp{} state={} result={} frame=cr{} adapter={} resume=@{} failure=@{}\n",
            id.into_raw().into_u32(),
            point.state,
            type_name(module, &point.result),
            point.frame.into_raw().into_u32(),
            module.classes[point.adapter].name,
            module.functions[point.resume].symbol,
            module.functions[point.resume_with_exception].symbol
        ));
    }
    for (id, coroutine) in module.meta.coroutine_functions.iter() {
        let lowering = match &coroutine.lowering {
            CoroutineLowering::Immediate => " immediate".to_string(),
            CoroutineLowering::StateMachine {
                frame,
                driver,
                resume_points,
            } => format!(
                " frame=cr{} driver=@{} resumes=[{}]",
                frame.into_raw().into_u32(),
                module.functions[*driver].symbol,
                resume_points
                    .iter()
                    .map(|id| format!("cp{}", id.into_raw().into_u32()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        };
        out.push_str(&format!(
            "  coroutine_fn cf{} @{} source_return={} step=cs{}{}\n",
            id.into_raw().into_u32(),
            module.functions[coroutine.function].symbol,
            type_name(module, &coroutine.source_return),
            coroutine.step.into_raw().into_u32(),
            lowering
        ));
    }
    for (_, instance) in module.meta.instances.iter() {
        out.push_str(&format!(
            "  instance @{} <- {}\n",
            instance.symbol, instance.source
        ));
    }
    for (_, string) in module.strings.iter() {
        out.push_str(&format!("  str @{} {:?}\n", string.symbol, string.value));
    }
    out.push_str(&format!("  entry @{ENTRY_SYMBOL}\n"));
    out
}

/// Render a type for dumps.
pub fn type_name(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Class(id) => module.classes[*id].name.clone(),
        Type::Interface(id) => module.interfaces[*id].name.clone(),
        Type::Any => "Any".to_string(),
        Type::Array(inner) => format!("Array<{}>", type_name(module, inner)),
        Type::MutableArray(inner) => format!("MutableArray<{}>", type_name(module, inner)),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| type_name(module, t)).collect();
            format!("({})", inner.join(", "))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<String> = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, ty))
                .collect();
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!(
                "{suspend}({}) -> {}",
                parameters.join(", "),
                type_name(module, &function.return_type)
            )
        }
        Type::Ptr(inner) => format!("Ptr<{}>", type_name(module, inner)),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<_> = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, ty))
                .collect();
            format!(
                "FunPtr<({}) -> {}>",
                parameters.join(", "),
                type_name(module, &function.return_type)
            )
        }
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(|t| type_name(module, t)).collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
    }
}

fn dump_statements(
    module: &Module,
    locals: &Arena<Local>,
    statements: &[Statement],
    indent: usize,
    out: &mut String,
) {
    for statement in statements {
        let pad = "  ".repeat(indent);
        match &statement.kind {
            StatementKind::Expr(expr) => dump_expr(module, locals, expr, indent, out),
            StatementKind::Call(effect) => match effect {
                CallEffect::Unit(call) => dump_call(module, locals, call, None, indent, out),
                CallEffect::Value { destination, call } => {
                    dump_call(module, locals, call, Some(*destination), indent, out)
                }
            },
            StatementKind::ValDecl { local, init } => {
                let local = &locals[*local];
                let keyword = if local.mutable { "var" } else { "val" };
                out.push_str(&format!(
                    "{pad}{keyword} {}: {}\n",
                    local.name,
                    type_name(module, &local.ty)
                ));
                dump_expr(module, locals, init, indent + 1, out);
            }
            StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}field_set {index}\n"));
                dump_expr(module, locals, object, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::ArraySet {
                array,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}array_set\n"));
                dump_expr(module, locals, array, indent + 1, out);
                dump_expr(module, locals, index, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::Assign { local, value } => {
                out.push_str(&format!("{pad}assign {}\n", locals[*local].name));
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::GlobalAssign { global, value } => {
                out.push_str(&format!(
                    "{pad}global_assign {}\n",
                    module.globals[*global].name
                ));
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::Eh(eh) => match eh {
                EhStatement::LandingPad { cleanup } => {
                    out.push_str(&format!("{pad}landing_pad cleanup={cleanup}\n"));
                }
                EhStatement::BeginCatch => out.push_str(&format!("{pad}begin_catch\n")),
                EhStatement::EndCatch => out.push_str(&format!("{pad}end_catch\n")),
            },
        }
    }
}

fn block_number(id: BlockId) -> u32 {
    id.into_raw().into_u32()
}

fn dump_terminator(
    module: &Module,
    locals: &Arena<Local>,
    terminator: &Terminator,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    match terminator {
        Terminator::Goto(target) => {
            out.push_str(&format!("{pad}goto bb{}\n", block_number(*target)));
        }
        Terminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            out.push_str(&format!(
                "{pad}branch bb{} bb{}\n",
                block_number(*then_block),
                block_number(*else_block)
            ));
            dump_expr(module, locals, cond, indent + 1, out);
        }
        Terminator::Return { value } => {
            out.push_str(&format!("{pad}return\n"));
            if let Some(value) = value {
                dump_expr(module, locals, value, indent + 1, out);
            }
        }
        Terminator::Throw { exception, unwind } => {
            let edge = unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!("{pad}throw{edge}\n"));
            dump_expr(module, locals, exception, indent + 1, out);
        }
        Terminator::Rethrow { unwind } => {
            let edge = unwind
                .map(|target| format!(" unwind bb{}", block_number(target)))
                .unwrap_or_default();
            out.push_str(&format!("{pad}rethrow{edge}\n"));
        }
        Terminator::Resume => out.push_str(&format!("{pad}resume\n")),
        Terminator::Trap { message } => {
            out.push_str(&format!("{pad}trap @{}\n", module.strings[*message].symbol));
        }
        Terminator::Unreachable => out.push_str(&format!("{pad}unreachable\n")),
    }
}

fn dump_expr(module: &Module, locals: &Arena<Local>, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringConst(id) => {
            out.push_str(&format!(
                "{pad}StringConst @{}\n",
                module.strings[*id].symbol
            ));
        }
        Expr::IntLiteral(value) => out.push_str(&format!("{pad}IntLiteral {value}\n")),
        Expr::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        Expr::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral\n")),
        Expr::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        Expr::ClassInit { class_id, args } => {
            out.push_str(&format!(
                "{pad}ClassInit {}\n",
                module.classes[*class_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        Expr::ClosureAlloc { class, captures } => {
            out.push_str(&format!(
                "{pad}ClosureAlloc cc{} {}\n",
                class.into_raw().into_u32(),
                module.closure_classes[*class].name
            ));
            for capture in captures {
                dump_expr(module, locals, capture, indent + 1, out);
            }
        }
        Expr::ClosureCapture {
            closure,
            class,
            index,
        } => {
            out.push_str(&format!(
                "{pad}ClosureCapture cc{} {index}\n",
                class.into_raw().into_u32()
            ));
            dump_expr(module, locals, closure, indent + 1, out);
        }
        Expr::StructInit { struct_id, args } => {
            out.push_str(&format!(
                "{pad}StructInit {}\n",
                module.structs[*struct_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        Expr::Local(local) => out.push_str(&format!("{pad}Local {}\n", locals[*local].name)),
        Expr::GlobalRead(global) => out.push_str(&format!(
            "{pad}GlobalRead {}\n",
            module.globals[*global].name
        )),
        Expr::PtrFromUInt { operand, pointee } => {
            out.push_str(&format!(
                "{pad}PtrFromUInt {}\n",
                type_name(module, pointee)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::PtrToUInt(operand) => {
            out.push_str(&format!("{pad}PtrToUInt\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::PtrCast { operand, pointee } => {
            out.push_str(&format!("{pad}PtrCast {}\n", type_name(module, pointee)));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::PtrLoad {
            pointer,
            pointee,
            offset,
        } => {
            out.push_str(&format!("{pad}PtrLoad {}\n", type_name(module, pointee)));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
        }
        Expr::PtrStore {
            pointer,
            pointee,
            offset,
            value,
        } => {
            out.push_str(&format!("{pad}PtrStore {}\n", type_name(module, pointee)));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
            dump_expr(module, locals, value, indent + 1, out);
        }
        Expr::PtrOffset {
            pointer,
            pointee,
            offset,
            subtract,
        } => {
            out.push_str(&format!(
                "{pad}PtrOffset {} subtract={subtract}\n",
                type_name(module, pointee)
            ));
            dump_expr(module, locals, pointer, indent + 1, out);
            dump_expr(module, locals, offset, indent + 1, out);
        }
        Expr::AddressOf { local, pointee } => out.push_str(&format!(
            "{pad}AddressOf {} : Ptr<{}>\n",
            locals[*local].name,
            type_name(module, pointee)
        )),
        Expr::GlobalAddress { global, pointee } => out.push_str(&format!(
            "{pad}GlobalAddress {} {}\n",
            module.globals[*global].name,
            type_name(module, pointee)
        )),
        Expr::SizeOf(ty) => {
            out.push_str(&format!("{pad}SizeOf {}\n", type_name(module, ty)));
        }
        Expr::AlignOf(ty) => {
            out.push_str(&format!("{pad}AlignOf {}\n", type_name(module, ty)));
        }
        Expr::FunPtrNull(signature) => out.push_str(&format!(
            "{pad}FunPtrNull function_type{}\n",
            signature.into_raw().into_u32()
        )),
        Expr::FunctionAddress { function, .. } => out.push_str(&format!(
            "{pad}FunctionAddress @{}\n",
            module.functions[*function].symbol
        )),
        Expr::CaughtException => out.push_str(&format!("{pad}CaughtException\n")),
        Expr::Retype { operand, ty } => {
            out.push_str(&format!("{pad}Retype {}\n", type_name(module, ty)));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::FieldAccess { receiver, index } => {
            out.push_str(&format!("{pad}FieldAccess {index}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        Expr::Box(operand) => {
            out.push_str(&format!("{pad}Box\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::Unbox(operand) => {
            out.push_str(&format!("{pad}Unbox\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::IsInstance { operand, check_ty } => {
            out.push_str(&format!(
                "{pad}IsInstance {}\n",
                type_name(module, check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::Cast { operand, optional } => {
            out.push_str(&format!("{pad}Cast optional={optional}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::ArrayLiteral(elements) => {
            out.push_str(&format!("{pad}ArrayLiteral\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        Expr::ArrayGet { array, index } => {
            out.push_str(&format!("{pad}ArrayGet\n"));
            dump_expr(module, locals, array, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
        }
        Expr::ArrayLen(operand) => {
            out.push_str(&format!("{pad}ArrayLen\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::ArrayClone(operand) => {
            out.push_str(&format!("{pad}ArrayClone\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        Expr::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::VariantConstruct {
            ty,
            variant,
            fields,
        } => {
            out.push_str(&format!(
                "{pad}VariantConstruct {} v{}\n",
                type_name(module, ty),
                variant
            ));
            for field in fields {
                dump_expr(module, locals, field, indent + 1, out);
            }
        }
        Expr::EnumTag(operand) => {
            out.push_str(&format!("{pad}EnumTag\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::EnumField {
            operand,
            variant,
            index,
        } => {
            out.push_str(&format!("{pad}EnumField v{variant} f{index}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}

fn dump_call(
    module: &Module,
    locals: &Arena<Local>,
    call: &Call,
    destination: Option<LocalId>,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    let callee = match &call.target.callee {
        Callee::User(id) => format!("@{}", module.functions[*id].symbol),
        Callee::Monomorphized(id) => format!("@{}", module.meta.instances[*id].symbol),
        Callee::Extern(id) => format!(
            "extern{} @{}",
            id.into_raw(),
            module.extern_functions[*id].native_symbol
        ),
        Callee::CoroutineSuspend { register } => format!(
            "@coroutine_suspend[register=@{}]",
            module.meta.instances[*register].symbol
        ),
        Callee::Closure(function_type) => {
            format!(
                "<closure:function_type{}>",
                function_type.into_raw().into_u32()
            )
        }
        Callee::FunctionBridge(function_type) => format!(
            "<function_bridge:function_type{}>",
            function_type.into_raw().into_u32()
        ),
        Callee::Runtime(function) => format!("@{}", function.symbol()),
    };
    let kind = match &call.target.kind {
        CallKind::Direct => "direct".to_string(),
        CallKind::Virtual { slot } => format!("virtual[{slot}]"),
        CallKind::Interface { interface, slot } => {
            format!("interface {}[{slot}]", module.interfaces[*interface].name)
        }
        CallKind::Closure { function_type } => {
            format!(
                "closure[function_type{}]",
                function_type.into_raw().into_u32()
            )
        }
        CallKind::FunctionBridge { function_type } => format!(
            "function_bridge[function_type{}]",
            function_type.into_raw().into_u32()
        ),
    };
    match destination {
        Some(local) => out.push_str(&format!(
            "{pad}call {}: {} = {callee} {kind}\n",
            locals[local].name,
            type_name(module, &locals[local].ty)
        )),
        None => out.push_str(&format!("{pad}call {callee} {kind}\n")),
    }
    for arg in &call.args {
        dump_expr(module, locals, arg, indent + 1, out);
    }
}
