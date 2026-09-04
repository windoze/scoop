use super::*;

#[derive(Debug)]
pub struct Module {
    pub functions: Arena<Function>,
    pub extern_functions: Arena<ExternFunction>,
    pub globals: Arena<Global>,
    pub initialization_units: Arena<InitializationUnit>,
    pub initialization_failure_roots: Arena<InitializationFailureRoot>,
    pub callback_bridges: Arena<CallbackBridge>,
    pub foreign_callback_adapters: Arena<ForeignCallbackAdapter>,
    pub foreign_callback_bridges: Arena<ForeignCallbackBridge>,
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

#[derive(Debug)]
pub struct CallbackBridge {
    pub source: FunctionId,
    pub signature: FunctionTypeId,
    /// NoGC storage-ABI entry called by the generated C trampoline.
    pub bridge_function: FunctionId,
}

/// Managed storage adapter for one foreign callback registration. Its
/// function is a GC-aware, nounwind boundary: it catches the closure's
/// exception and returns a runtime status instead of unwinding into C.
#[derive(Debug)]
pub struct ForeignCallbackAdapter {
    pub function: FunctionId,
    pub managed_signature: FunctionTypeId,
}

/// Native-signature side of one typed managed callback registration.
/// Adapter and bridge ids are intentionally distinct from M12's static
/// `CallbackBridgeId` so a closure can never enter the NoGC callback path.
#[derive(Debug)]
pub struct ForeignCallbackBridge {
    pub adapter: ForeignCallbackAdapterId,
    pub callback: StructId,
    pub native_signature: FunctionTypeId,
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

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    pub symbol: String,
    pub ty: Type,
    pub mutable: bool,
    pub storage: GlobalStorage,
}

#[derive(Debug)]
pub struct InitializationUnit {
    pub stable_key: String,
    pub kind: InitializationUnitKind,
    pub initializer: FunctionId,
    pub ensure: FunctionId,
    pub failure_root: InitializationFailureRootId,
    pub dependencies: Vec<InitializationUnitId>,
    pub cycle_exception: MessageClassConstructor,
}

#[derive(Debug, Clone)]
pub struct MessageClassConstructor {
    pub class: ClassId,
    pub initializer: FunctionId,
    pub message_type: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationUnitKind {
    EagerTopLevel { storage: GlobalId },
}

#[derive(Debug)]
pub struct InitializationFailureRoot {
    pub global: GlobalId,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Managed {
        initializer: ConstantValue,
    },
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
    Zero,
    Int(i64),
    Bool(bool),
    String(StringConstId),
    NullPtr,
    NullFunPtr,
    EnumUnit {
        enum_id: EnumId,
        variant: u32,
    },
    Struct {
        struct_id: StructId,
        fields: Vec<ConstantValue>,
    },
}

/// Per-Cone MIR metadata (impl spec 2.3).
#[derive(Debug, Default)]
pub struct MirMeta {
    pub dispatch_tables: Vec<DispatchTable>,
    /// Typed source identities are separate from their display names and from
    /// concrete instances. The three id families cannot be interchanged.
    pub generic_function_sources: Arena<GenericFunctionSource>,
    pub parameterized_method_sources: Arena<ParameterizedMethodSource>,
    pub generic_method_sources: Arena<GenericMethodSource>,
    /// Monomorphized function instances in creation order. The entry
    /// records the emitted symbol and its MIR-local typed source identity.
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
    /// Complete semantic relation between every materialized value box and
    /// the concrete class whose TypeDescriptor represents it. LIR must not
    /// reconstruct this relation from the synthetic class link name.
    pub boxed_types: Vec<BoxedType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoxedType {
    pub payload: Type,
    pub class: ClassId,
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

/// Display metadata for one generic free-function declaration. Identity is
/// the arena id; `display_name` is never used for semantic decisions.
#[derive(Debug)]
pub struct GenericFunctionSource {
    pub display_name: String,
}

/// Display metadata for one ordinary member whose owner is generic.
#[derive(Debug)]
pub struct ParameterizedMethodSource {
    pub display_name: String,
}

/// Display metadata for one method that declares its own type parameters.
#[derive(Debug)]
pub struct GenericMethodSource {
    pub display_name: String,
}

/// A structurally non-empty MIR type-argument group.
#[derive(Debug, Clone, PartialEq)]
pub struct NonEmptyTypeArguments {
    first: Type,
    rest: Vec<Type>,
}

impl NonEmptyTypeArguments {
    pub fn new(first: Type, rest: Vec<Type>) -> Self {
        Self { first, rest }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Type> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    pub fn to_vec(&self) -> Vec<Type> {
        self.iter().cloned().collect()
    }
}

/// Exact concrete owner of a monomorphized method instance.
#[derive(Debug, Clone, PartialEq)]
pub enum MonomorphizedMethodOwner {
    Class(ClassId),
    Struct(StructId),
    Enum(EnumId),
    Interface(InterfaceId),
    Structural(Type),
}

/// Typed provenance and structurally complete argument groups for one
/// concrete instance. Owner and method arguments cannot be flattened or
/// attached to the wrong source category.
#[derive(Debug, Clone, PartialEq)]
pub enum MonomorphizedSource {
    GenericFunction {
        source: GenericFunctionSourceId,
        arguments: NonEmptyTypeArguments,
    },
    ParameterizedMethod {
        source: ParameterizedMethodSourceId,
        owner: MonomorphizedMethodOwner,
        owner_arguments: NonEmptyTypeArguments,
    },
    GenericMethod {
        source: GenericMethodSourceId,
        owner: MonomorphizedMethodOwner,
        owner_arguments: Vec<Type>,
        method_arguments: NonEmptyTypeArguments,
    },
}

impl MirMeta {
    /// Human-readable source name for dumps and diagnostics only.
    pub fn monomorphized_source_display_name(&self, source: &MonomorphizedSource) -> &str {
        match source {
            MonomorphizedSource::GenericFunction { source, .. } => {
                &self.generic_function_sources[*source].display_name
            }
            MonomorphizedSource::ParameterizedMethod { source, .. } => {
                &self.parameterized_method_sources[*source].display_name
            }
            MonomorphizedSource::GenericMethod { source, .. } => {
                &self.generic_method_sources[*source].display_name
            }
        }
    }
}

/// Provenance of one concrete generic function emitted into the MIR function
/// arena. Its typed id is also what MIR call sites carry.
#[derive(Debug)]
pub struct MonomorphizedFunction {
    pub function: FunctionId,
    pub symbol: String,
    pub source: MonomorphizedSource,
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
