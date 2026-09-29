use super::*;
use crate::ImportedCoreProtocolCallable;

#[derive(Debug, Clone)]
pub struct Module {
    /// Producer Cone preserved from Export HIR. Generated definitions must
    /// use this identity rather than a request-local source or arena id.
    pub cone: scoop_identity::ConeIdentity,
    pub types: Arena<Type>,
    /// Total persistent identity relation aligned with `types`. Local-
    /// concrete HIR cannot represent an open type, so every entry is exact.
    pub exact_type_identities: ExactTypeIdentities,
    /// Total persistent identity relation for every callable-local value in
    /// this fully materialized graph.
    pub local_value_identities: LocalValueIdentities,
    /// Total mapping from LocalConcrete virtual/interface slot ids to their
    /// declaration-level persistent identities.
    pub dispatch_slot_identities: DispatchSlotIdentities,
    /// Canonical application identities referenced by callable
    /// materializations in this local graph.
    pub callable_applications: CallableApplicationIdentities,
    /// Generated keys whose exact owners become known during concretization.
    pub generated_callable_identities: Vec<GeneratedCallableRecord>,
    /// Canonical callback applications referenced by concrete callback
    /// registrations in this local graph.
    pub callback_applications: CallbackApplicationIdentities,
    pub function_types: Arena<FunctionType>,
    pub lambdas: Arena<Lambda>,
    pub anonymous_functions: Arena<AnonymousFunction>,
    pub local_functions: Arena<LocalFunction>,
    pub callable_references: Arena<CallableReference>,
    /// Ordinary dependency callable uses transposed one-to-one from Export
    /// HIR into their own LocalConcrete arena-id domain.
    pub imported_dependency_callables: Arena<ImportedDependencyCallableUse>,
    pub function_coercions: Arena<FunctionCoercion>,
    pub foreign_callback_registrations: Arena<ForeignCallbackRegistration>,
    pub functions: Arena<Function>,
    pub extern_functions: Arena<ExternFunction>,
    pub globals: Arena<Global>,
    pub generic_delegate_specializations: Arena<GenericDelegateStorageSpecialization>,
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
    /// The single compiler-protocol authority for this LocalConcrete graph.
    ///
    /// Bootstrap graphs define the complete protocol product locally;
    /// ordinary graphs retain the imported authority without copying core
    /// declarations into this Cone's arenas.
    pub core_protocols: ConcreteCoreProtocols,
}

impl Module {
    pub fn option_core(&self, enumeration: EnumId) -> Option<OptionCore> {
        match &self.core_protocols {
            ConcreteCoreProtocols::Defined(protocols) => protocols
                .option
                .iter()
                .copied()
                .find(|option| option.enumeration() == enumeration),
            ConcreteCoreProtocols::Imported(protocols) => {
                let protocol = protocols.option();
                let definition = &self.enums[enumeration];
                if definition.origin.generic_type_id() != Some(protocol.option().persistent()) {
                    return None;
                }
                let variant = |identity| {
                    let index = definition
                        .variants
                        .iter()
                        .position(|variant| variant.identity == identity)?;
                    EnumVariantRef::checked(
                        &self.enums,
                        enumeration,
                        VariantId::from_raw(index as u32),
                    )
                };
                let some = variant(protocol.some().persistent())?;
                let none = variant(protocol.none().persistent())?;
                let field = definition.variants[some.variant().into_raw() as usize]
                    .fields
                    .iter()
                    .position(|field| field.identity == protocol.some_payload().persistent())?;
                let payload = EnumVariantFieldRef::checked(&self.enums, some, field as u32)?;
                OptionCore::checked(&self.enums, payload, none)
            }
        }
    }
}

/// Closed compiler-protocol authority carried by one LocalConcrete HIR graph.
///
/// The variants are intentionally disjoint: imported persistent subjects
/// cannot be represented as declarations owned by the current Cone.
#[derive(Debug, Clone)]
pub enum ConcreteCoreProtocols {
    Defined(Box<DefinedConcreteCoreProtocols>),
    Imported(Box<crate::ImportedCoreProtocols>),
}

/// Closed local-concrete compiler protocol product. Its fields are kept
/// together so a defining graph cannot replace or omit one protocol family
/// independently of the others.
#[derive(Debug, Clone)]
pub struct DefinedConcreteCoreProtocols {
    pub option: Vec<OptionCore>,
    pub exceptions: CompilerExceptionCore,
    pub coroutines: Vec<CoroutineProtocol>,
    pub foreign_callbacks: ForeignCallbackCore,
    /// Nominal owners of the fixed compiler-represented types. Generic
    /// intrinsic families are represented by each concrete class instance,
    /// so no parameterized template can leak into this local graph.
    pub fundamental_types: IntrinsicTypeCore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroArgClassConstructor {
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
    pub initialization_cycle_thrower: FunctionId,
}

#[derive(Debug, Clone)]
pub struct InitializationUnit {
    pub identity: InitializationUnitIdentityRecord,
    pub display_name: String,
    pub schedule: InitializationSchedule,
    pub kind: InitializationUnitKind,
    pub initializer: FunctionId,
    pub ensure: FunctionId,
    pub failure_root: InitializationFailureRootId,
    pub dependencies: Vec<InitializationDependency>,
    pub cycle_thrower: InitializationCycleThrower,
}

/// Fully selected cycle-error exit for one concrete initialization unit.
/// Retains a local function or the actual dependency callable declaration.
#[derive(Debug, Clone)]
pub enum InitializationCycleThrower {
    Local(FunctionId),
    Imported(ImportedCoreProtocolCallable),
}

pub type InitializationUnitIdentityRecord =
    CborIdentityRecord<PersistentInitializationUnitId, InitializationUnitKey>;

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
    GenericDelegatedExtension {
        specialization: GenericDelegateStorageSpecializationId,
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
    /// Persistent identity of this fully concrete callback conversion.
    pub application: PersistentCallbackApplicationId,
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
        let ConcreteCoreProtocols::Defined(protocols) = &self.core_protocols else {
            return None;
        };
        protocols.coroutines.iter().find(|protocol| {
            protocol.start_coroutine == function || protocol.suspend_coroutine == function
        })
    }
}
