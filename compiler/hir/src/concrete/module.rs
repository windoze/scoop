use super::*;

#[derive(Debug, Clone)]
pub struct Module {
    pub types: Arena<Type>,
    pub function_types: Arena<FunctionType>,
    pub lambdas: Arena<Lambda>,
    pub anonymous_functions: Arena<AnonymousFunction>,
    pub local_functions: Arena<LocalFunction>,
    pub callable_references: Arena<CallableReference>,
    pub function_coercions: Arena<FunctionCoercion>,
    pub foreign_callback_registrations: Arena<ForeignCallbackRegistration>,
    pub functions: Arena<Function>,
    pub extern_functions: Arena<ExternFunction>,
    pub globals: Arena<Global>,
    pub initialization_units: Arena<InitializationUnit>,
    pub initialization_failure_roots: Arena<InitializationFailureRoot>,
    pub structs: Arena<StructDef>,
    pub enums: Arena<EnumDef>,
    pub classes: Arena<ClassDef>,
    /// Hidden, fully typed allocation/initialization callables for every
    /// instantiable declared class. Class construction expressions and
    /// compiler exceptions reference these ids directly.
    pub class_constructors: Arena<ClassConstructor>,
    /// Fully specialized value-returning constructor callables. Primary and
    /// secondary constructors retain distinct typed identities.
    pub struct_constructors: Arena<StructConstructor>,
    pub interfaces: Arena<InterfaceDef>,
    pub top_level: Vec<FunctionId>,
    pub unit: TypeId,
    pub int: TypeId,
    pub boolean: TypeId,
    pub string: TypeId,
    pub option_variants: (VariantId, VariantId),
    pub exception_core: CompilerExceptionCore,
    pub coroutine_protocols: Vec<CoroutineProtocol>,
    pub foreign_callback_core: ForeignCallbackCore,
    /// Nominal owners of the fixed compiler-represented types. Generic
    /// intrinsic families are represented by each concrete class instance,
    /// so no parameterized template can leak into this local graph.
    pub intrinsic_type_core: IntrinsicTypeCore,
    pub entry: FunctionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroArgClassConstructor {
    pub class: ClassId,
    pub callable: ClassConstructorId,
}

/// A constructor whose only physical parameter is the exact core
/// `Option<String>` application used for initialization-cycle messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageClassConstructor {
    pub class: ClassId,
    pub callable: ClassConstructorId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerException {
    pub constructor: ZeroArgClassConstructor,
}

impl CompilerException {
    pub const fn class(self) -> ClassId {
        self.constructor.class
    }

    pub const fn callable(self) -> ClassConstructorId {
        self.constructor.callable
    }
}

/// Local-concrete exception capabilities. Export ids cannot be represented
/// here, and every constructor target has already been fully specialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerExceptionCore {
    pub throwable: CompilerException,
    pub unwrap_exception: CompilerException,
    pub class_cast_exception: CompilerException,
    pub arithmetic_exception: CompilerException,
    pub index_out_of_bounds_exception: CompilerException,
    pub illegal_state_exception: CompilerException,
    pub illegal_state_message_constructor: MessageClassConstructor,
}

#[derive(Debug, Clone)]
pub struct InitializationUnit {
    pub stable_key: String,
    pub kind: InitializationUnitKind,
    pub initializer: FunctionId,
    pub ensure: FunctionId,
    pub failure_root: InitializationFailureRootId,
    pub dependencies: Vec<InitializationDependency>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationUnitKind {
    EagerTopLevel { storage: GlobalId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializationDependency {
    pub unit: InitializationUnitId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitializationFailureRoot {
    pub unit: InitializationUnitId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackCore {
    pub mode: EnumId,
    pub state: EnumId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicTypeCore {
    pub int: StructId,
    pub uint: StructId,
    pub boolean: StructId,
    pub string: ClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackMode {
    Reusable,
    OneShot,
}

#[derive(Debug, Clone)]
pub struct ForeignCallbackRegistration {
    pub callback: StructId,
    pub native_function_type: FunctionTypeId,
    pub managed_function_type: FunctionTypeId,
    pub context_index: u32,
    pub mode: ForeignCallbackMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackOperation {
    Retain,
    Release,
    State,
    Failure,
}

/// Fully specialized instances of the generic coroutine protocol for one
/// result type.  MIR consumes these concrete identities and never reads the
/// export-side protocol templates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoroutineProtocol {
    pub result_type: TypeId,
    pub continuation: InterfaceId,
    pub suspend_task: InterfaceId,
    pub suspend_registration: InterfaceId,
    pub start_coroutine: FunctionId,
    pub suspend_coroutine: FunctionId,
    pub continuation_resume: FunctionId,
    pub continuation_resume_with_exception: FunctionId,
    pub suspend_task_run: FunctionId,
    pub suspend_registration_register: FunctionId,
}

impl Module {
    pub fn callable_function(&self, callable: Callable) -> FunctionId {
        match callable {
            Callable::Function(function) => function,
        }
    }

    pub fn coroutine_protocol_for_function(
        &self,
        function: FunctionId,
    ) -> Option<&CoroutineProtocol> {
        self.coroutine_protocols.iter().find(|protocol| {
            protocol.start_coroutine == function || protocol.suspend_coroutine == function
        })
    }
}
