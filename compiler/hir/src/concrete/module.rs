use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Module {
    /// Cone identity of the compilation; feeds generated-entity keys.
    pub cone: scoop_identity::ConeIdentity,
    pub types: Arena<Type>,
    /// Exact-type identity of every interned type; consumed by MIR/LIR
    /// for specializations and runtime type identity.
    pub exact_of: HashMap<TypeId, scoop_identity::persistent::PersistentExactTypeId>,
    pub exact_types: scoop_identity::persistent::ExactTypeTable,
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
    pub objects: Arena<ObjectDecl>,
    pub object_types: Arena<ObjectType>,
    pub companion_relations: Arena<CompanionRelation>,
    pub singleton_values: Arena<SingletonValue>,
    pub singleton_published_roots: Arena<SingletonPublishedRoot>,
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
    pub boolean: TypeId,
    pub string: TypeId,
    pub option_core: Vec<OptionCore>,
    pub exception_core: CompilerExceptionCore,
    pub coroutine_protocols: Vec<CoroutineProtocol>,
    pub foreign_callback_core: ForeignCallbackCore,
    /// Nominal owners of the fixed compiler-represented types. Generic
    /// intrinsic families are represented by each concrete class instance,
    /// so no parameterized template can leak into this local graph.
    pub intrinsic_type_core: IntrinsicTypeCore,
    pub entry: FunctionId,
}

impl Module {
    pub fn option_core(&self, enumeration: EnumId) -> Option<OptionCore> {
        self.option_core
            .iter()
            .copied()
            .find(|option| option.enumeration() == enumeration)
    }
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
    pub schedule: InitializationSchedule,
    pub kind: InitializationUnitKind,
    pub initializer: FunctionId,
    pub ensure: FunctionId,
    pub failure_root: InitializationFailureRootId,
    pub dependencies: Vec<InitializationDependency>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationSchedule {
    EagerStartup,
    LazyAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationUnitKind {
    EagerTopLevel {
        storage: GlobalId,
    },
    LazySingleton {
        value: SingletonValueId,
        published_root: SingletonPublishedRootId,
    },
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
    pub modes: ForeignCallbackModes,
    pub states: ForeignCallbackStates,
    pub failure_result: ForeignCallbackFailureResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackModes {
    reusable: EnumVariantRef,
    one_shot: EnumVariantRef,
}

impl ForeignCallbackModes {
    pub fn checked(
        enums: &Arena<EnumDef>,
        reusable: EnumVariantRef,
        one_shot: EnumVariantRef,
    ) -> Option<Self> {
        let reusable = EnumVariantRef::checked(enums, reusable.enumeration(), reusable.variant())?;
        let one_shot = EnumVariantRef::checked(enums, one_shot.enumeration(), one_shot.variant())?;
        if reusable.enumeration() != one_shot.enumeration() {
            return None;
        }
        let enumeration = &enums[reusable.enumeration()];
        let reusable_definition = &enumeration.variants[reusable.variant().into_raw() as usize];
        let one_shot_definition = &enumeration.variants[one_shot.variant().into_raw() as usize];
        (enumeration.name == "ForeignCallbackMode"
            && enumeration.type_arguments.is_empty()
            && enumeration.variants.len() == 2
            && reusable.variant().into_raw() == 0
            && one_shot.variant().into_raw() == 1
            && reusable_definition.name == "Reusable"
            && reusable_definition.fields.is_empty()
            && one_shot_definition.name == "OneShot"
            && one_shot_definition.fields.is_empty())
        .then_some(Self { reusable, one_shot })
    }

    pub const fn reusable(self) -> EnumVariantRef {
        self.reusable
    }

    pub const fn one_shot(self) -> EnumVariantRef {
        self.one_shot
    }

    pub const fn enumeration(self) -> EnumId {
        self.reusable.enumeration()
    }

    pub fn contains(self, variant: EnumVariantRef) -> bool {
        variant == self.reusable || variant == self.one_shot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackStates {
    registered: EnumVariantRef,
    active: EnumVariantRef,
    completed: EnumVariantRef,
    failed: EnumVariantRef,
}

impl ForeignCallbackStates {
    pub fn checked(
        enums: &Arena<EnumDef>,
        registered: EnumVariantRef,
        active: EnumVariantRef,
        completed: EnumVariantRef,
        failed: EnumVariantRef,
    ) -> Option<Self> {
        let variants = [registered, active, completed, failed].map(|variant| {
            EnumVariantRef::checked(enums, variant.enumeration(), variant.variant())
        });
        let [
            Some(registered),
            Some(active),
            Some(completed),
            Some(failed),
        ] = variants
        else {
            return None;
        };
        let variants = [registered, active, completed, failed];
        if variants
            .iter()
            .any(|variant| variant.enumeration() != registered.enumeration())
        {
            return None;
        }
        let enumeration = &enums[registered.enumeration()];
        let expected = [
            (registered, "Registered"),
            (active, "Active"),
            (completed, "Completed"),
            (failed, "Failed"),
        ];
        (enumeration.name == "ForeignCallbackState"
            && enumeration.type_arguments.is_empty()
            && enumeration.variants.len() == 4
            && expected.iter().enumerate().all(|(index, (variant, name))| {
                variant.variant().into_raw() as usize == index
                    && enumeration.variants[index].name == *name
                    && enumeration.variants[index].fields.is_empty()
            }))
        .then_some(Self {
            registered,
            active,
            completed,
            failed,
        })
    }

    pub const fn registered(self) -> EnumVariantRef {
        self.registered
    }

    pub const fn active(self) -> EnumVariantRef {
        self.active
    }

    pub const fn completed(self) -> EnumVariantRef {
        self.completed
    }

    pub const fn failed(self) -> EnumVariantRef {
        self.failed
    }

    pub const fn enumeration(self) -> EnumId {
        self.registered.enumeration()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackFailureResult {
    option: OptionCore,
    throwable: ClassId,
}

impl ForeignCallbackFailureResult {
    pub fn checked(
        enums: &Arena<EnumDef>,
        types: &Arena<Type>,
        option: OptionCore,
        throwable: ClassId,
    ) -> Option<Self> {
        let option = OptionCore::checked(enums, option.some_payload(), option.none())?;
        let payload = &enums[option.enumeration()].variants
            [option.some().variant().into_raw() as usize]
            .fields[option.some_payload().local_index() as usize];
        if payload.ty.into_raw().into_u32() as usize >= types.len()
            || !matches!(types[payload.ty].kind, TypeKind::Class(found) if found == throwable)
        {
            return None;
        }
        Some(Self { option, throwable })
    }

    pub const fn some_payload(self) -> EnumVariantFieldRef {
        self.option.some_payload()
    }

    pub const fn none(self) -> EnumVariantRef {
        self.option.none()
    }

    pub const fn enumeration(self) -> EnumId {
        self.option.enumeration()
    }

    pub const fn throwable(self) -> ClassId {
        self.throwable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicTypeCore {
    pub integers: IntegerTypeCore<StructId>,
    pub boolean: StructId,
    pub string: ClassId,
}

#[derive(Debug, Clone)]
pub struct ForeignCallbackRegistration {
    pub callback: StructId,
    pub native_function_type: FunctionTypeId,
    pub managed_function_type: FunctionTypeId,
    pub context_index: u32,
    pub mode: EnumVariantRef,
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
