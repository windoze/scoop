use super::*;

fn arena_get<T>(arena: &Arena<T>, id: Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}

#[derive(Debug)]
pub struct Module {
    pub functions: Arena<Function>,
    pub extern_functions: Arena<ExternFunction>,
    pub globals: Arena<Global>,
    pub initialization_units: Arena<InitializationUnit>,
    pub initialization_failure_roots: Arena<InitializationFailureRoot>,
    pub objects: Arena<ObjectDef>,
    pub object_types: Arena<ObjectType>,
    pub singleton_values: Arena<SingletonValue>,
    pub singleton_published_roots: Arena<SingletonPublishedRoot>,
    pub callback_bridges: Arena<CallbackBridge>,
    pub foreign_callback_adapters: Arena<ForeignCallbackAdapter>,
    pub foreign_callback_families: Arena<ForeignCallbackFamily>,
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
    /// Every concrete specialization of core `Option`, identified by its
    /// inseparable Some payload-field and None variant identities. Consumers
    /// must use this registry instead of recognizing nullable layouts by shape.
    pub option_core: Vec<OptionCore>,
    pub entry: FunctionId,
    pub meta: MirMeta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptionCore {
    some_payload: MirVariantFieldRef,
    none: MirVariantRef,
}

impl OptionCore {
    pub fn checked(
        enums: &Arena<EnumDef>,
        some_payload: MirVariantFieldRef,
        none: MirVariantRef,
    ) -> Option<Self> {
        let some = some_payload.variant();
        if some.enum_id() != none.enum_id() || some.variant_index() == none.variant_index() {
            return None;
        }
        let some_definition = some.definition(enums).ok()?;
        let none_definition = none.definition(enums).ok()?;
        let enumeration = &enums[some.enum_id()];
        let [argument] = enumeration.type_arguments.as_slice() else {
            return None;
        };
        (enumeration.variants.len() == 2
            && some_definition.name == "Some"
            && some_definition.fields.len() == 1
            && some_payload.field_index() == 0
            && some_payload.definition(enums).ok()?.ty == *argument
            && none_definition.name == "None"
            && none_definition.fields.is_empty())
        .then_some(Self { some_payload, none })
    }

    pub const fn enum_id(self) -> EnumId {
        self.some_payload.variant().enum_id()
    }

    pub const fn some(self) -> MirVariantRef {
        self.some_payload.variant()
    }

    pub const fn some_payload(self) -> MirVariantFieldRef {
        self.some_payload
    }

    pub const fn none(self) -> MirVariantRef {
        self.none
    }
}

impl Module {
    pub fn option_core(&self, enum_id: EnumId) -> Option<&OptionCore> {
        self.option_core
            .iter()
            .find(|option| option.enum_id() == enum_id)
    }
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

/// One concrete `ForeignCallback<F>` protocol family. The record atomically
/// binds the callback value, state result, and failure result to their exact
/// nominal identities; callback instructions carry only this typed id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackFamily {
    pub callback: StructId,
    pub modes: ForeignCallbackModes,
    pub states: ForeignCallbackStates,
    pub failure_result: ForeignCallbackFailureResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackModes {
    reusable: MirVariantRef,
    one_shot: MirVariantRef,
}

impl ForeignCallbackModes {
    pub fn checked(
        enums: &Arena<EnumDef>,
        reusable: MirVariantRef,
        one_shot: MirVariantRef,
    ) -> Option<Self> {
        let reusable_definition = reusable.definition(enums).ok()?;
        let one_shot_definition = one_shot.definition(enums).ok()?;
        if reusable.enum_id() != one_shot.enum_id() {
            return None;
        }
        let enumeration = &enums[reusable.enum_id()];
        (enumeration.name == "ForeignCallbackMode"
            && enumeration.type_arguments.is_empty()
            && enumeration.variants.len() == 2
            && reusable.variant_index() == 0
            && one_shot.variant_index() == 1
            && reusable_definition.name == "Reusable"
            && reusable_definition.fields.is_empty()
            && one_shot_definition.name == "OneShot"
            && one_shot_definition.fields.is_empty())
        .then_some(Self { reusable, one_shot })
    }

    pub const fn reusable(self) -> MirVariantRef {
        self.reusable
    }

    pub const fn one_shot(self) -> MirVariantRef {
        self.one_shot
    }

    pub const fn enum_id(self) -> EnumId {
        self.reusable.enum_id()
    }

    pub fn contains(self, variant: MirVariantRef) -> bool {
        variant == self.reusable || variant == self.one_shot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackStates {
    registered: MirVariantRef,
    active: MirVariantRef,
    completed: MirVariantRef,
    failed: MirVariantRef,
}

impl ForeignCallbackStates {
    pub fn checked(
        enums: &Arena<EnumDef>,
        registered: MirVariantRef,
        active: MirVariantRef,
        completed: MirVariantRef,
        failed: MirVariantRef,
    ) -> Option<Self> {
        let variants = [registered, active, completed, failed];
        if variants
            .iter()
            .any(|variant| variant.enum_id() != registered.enum_id())
        {
            return None;
        }
        if registered.enum_id().into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        let enumeration = &enums[registered.enum_id()];
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
                variant.variant_index() as usize == index
                    && variant.definition(enums).is_ok_and(|definition| {
                        definition.name == *name && definition.fields.is_empty()
                    })
            }))
        .then_some(Self {
            registered,
            active,
            completed,
            failed,
        })
    }

    pub const fn registered(self) -> MirVariantRef {
        self.registered
    }

    pub const fn active(self) -> MirVariantRef {
        self.active
    }

    pub const fn completed(self) -> MirVariantRef {
        self.completed
    }

    pub const fn failed(self) -> MirVariantRef {
        self.failed
    }

    pub const fn enum_id(self) -> EnumId {
        self.registered.enum_id()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackFailureResult {
    option: OptionCore,
    throwable: ClassId,
}

impl ForeignCallbackFailureResult {
    pub fn checked(enums: &Arena<EnumDef>, option: OptionCore, throwable: ClassId) -> Option<Self> {
        let option = OptionCore::checked(enums, option.some_payload(), option.none())?;
        let payload = option.some_payload().definition(enums).ok()?;
        (payload.ty == Type::Class(throwable)).then_some(Self { option, throwable })
    }

    pub const fn some_payload(self) -> MirVariantFieldRef {
        self.option.some_payload()
    }

    pub const fn none(self) -> MirVariantRef {
        self.option.none()
    }

    pub const fn enum_id(self) -> EnumId {
        self.option.enum_id()
    }

    pub const fn throwable(self) -> ClassId {
        self.throwable
    }
}

/// Native-signature side of one typed managed callback registration.
/// Adapter and bridge ids are intentionally distinct from M12's static
/// `CallbackBridgeId` so a closure can never enter the NoGC callback path.
#[derive(Debug)]
pub struct ForeignCallbackBridge {
    pub adapter: ForeignCallbackAdapterId,
    pub family: ForeignCallbackFamilyId,
    pub native_signature: FunctionTypeId,
    pub context_index: u32,
    pub mode: MirVariantRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackOperation {
    Retain(ForeignCallbackFamilyId),
    Release(ForeignCallbackFamilyId),
    State(ForeignCallbackFamilyId),
    Failure(ForeignCallbackFamilyId),
}

impl ForeignCallbackOperation {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Retain(_) => "Retain",
            Self::Release(_) => "Release",
            Self::State(_) => "State",
            Self::Failure(_) => "Failure",
        }
    }

    pub const fn family(self) -> ForeignCallbackFamilyId {
        match self {
            Self::Retain(family)
            | Self::Release(family)
            | Self::State(family)
            | Self::Failure(family) => family,
        }
    }
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
    pub schedule: InitializationSchedule,
    pub kind: InitializationUnitKind,
    pub initializer: FunctionId,
    pub ensure: FunctionId,
    pub failure_root: InitializationFailureRootId,
    pub dependencies: Vec<InitializationUnitId>,
    pub cycle_exception: MessageClassConstructor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationSchedule {
    EagerStartup,
    LazyAccess,
}

#[derive(Debug, Clone)]
pub struct MessageClassConstructor {
    pub class: ClassId,
    pub initializer: FunctionId,
    pub message_type: Type,
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

#[derive(Debug)]
pub struct InitializationFailureRoot {
    pub global: GlobalId,
}

#[derive(Debug)]
pub struct ObjectDef {
    pub name: String,
    pub object_type: ObjectTypeId,
    pub singleton_value: SingletonValueId,
    pub backing_class: ClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectType {
    pub declaration: ObjectId,
    pub representation: ClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingletonValue {
    pub declaration: ObjectId,
    pub object_type: ObjectTypeId,
    pub published_root: SingletonPublishedRootId,
    pub initialization: InitializationUnitId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingletonPublishedRoot {
    pub value: SingletonValueId,
    pub global: GlobalId,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Managed {
        initial_state: MirStaticInitialState,
    },
    Local {
        thread_local: bool,
        initial_state: MirStaticInitialState,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

/// Pointer-null provenance in a layout-independent MIR constant image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirPointerNull {
    Data,
    Code,
}

/// Layout-independent, recursively typed static image.
///
/// There is deliberately no generic zero or bare integer branch. Integer
/// zero, pointer null, and a runtime-owned zeroed storage unit remain three
/// different semantic states.
#[derive(Debug, Clone, PartialEq)]
pub enum MirConstantImage {
    Integer(MirIntegerConstant),
    Boolean(bool),
    String(StringConstId),
    PointerNull(MirPointerNull),
    EnumUnit {
        variant: MirVariantRef,
    },
    Struct {
        struct_id: StructId,
        fields: Vec<MirConstantImage>,
    },
}

/// Initial state of one compiler-owned static storage object.
#[derive(Debug, Clone, PartialEq)]
pub enum MirStaticInitialState {
    ZeroedForRuntimeUnit,
    EncodedStaticValue { payload: MirConstantImage },
}

/// Constant metadata retained from a validated source annotation.
///
/// Compiler-recognized annotations normally normalize into dedicated sums
/// (for example [`MirCLayoutContract`]); annotations that remain as metadata
/// cannot lose an integer's exact signedness or width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirAnnotationValue {
    Integer(MirIntegerConstant),
    Boolean(bool),
    String(String),
}

/// Per-Cone MIR metadata (impl spec 2.3).
#[derive(Debug, Default)]
pub struct MirMeta {
    /// Exact schema used by every symbol in this module and exported MIR meta.
    pub mangling_schema: ManglingSchemaIdentity,
    /// Exact-type identity of every MIR type lowered from concrete HIR,
    /// keyed structurally. Generated nominals (closures, adapters, boxed
    /// classes) join with their generated-role keys as their sites are
    /// migrated; consumers must treat a missing entry as "no persistent
    /// identity yet", never guess one.
    pub exact_of: std::collections::HashMap<
        crate::types::Type,
        scoop_identity::persistent::PersistentExactTypeId,
    >,
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
    /// Exact dynamic values retained by a pending return or managed throw.
    pub coroutine_saved_values: Arena<CoroutineSavedValue>,
    /// Exact managed throwable slot used by continuation failure injection.
    pub coroutine_failure_values: Arena<CoroutineFailureValue>,
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
    completed_payload: MirVariantFieldRef,
    suspended: MirVariantRef,
    result: Type,
}

impl CoroutineStep {
    pub fn checked(
        enums: &Arena<EnumDef>,
        completed_payload: MirVariantFieldRef,
        suspended: MirVariantRef,
        result: Type,
    ) -> Option<Self> {
        let completed = completed_payload.variant();
        if completed.enum_id() != suspended.enum_id()
            || completed.variant_index() == suspended.variant_index()
        {
            return None;
        }
        let completed_definition = completed.definition(enums).ok()?;
        let suspended_definition = suspended.definition(enums).ok()?;
        (enums[completed.enum_id()].type_arguments.is_empty()
            && enums[completed.enum_id()].variants.len() == 2
            && completed_definition.fields.len() == 1
            && completed_payload.field_index() == 0
            && completed_payload.definition(enums).ok()?.ty == result
            && suspended_definition.fields.is_empty())
        .then_some(Self {
            completed_payload,
            suspended,
            result,
        })
    }

    pub const fn enum_id(&self) -> EnumId {
        self.completed_payload.variant().enum_id()
    }

    pub const fn completed(&self) -> MirVariantRef {
        self.completed_payload.variant()
    }

    pub const fn completed_payload(&self) -> MirVariantFieldRef {
        self.completed_payload
    }

    pub const fn suspended(&self) -> MirVariantRef {
        self.suspended
    }

    pub const fn result(&self) -> &Type {
        &self.result
    }
}

#[derive(Debug)]
pub struct CoroutineSlot {
    value_payload: MirVariantFieldRef,
    empty: MirVariantRef,
    value: Type,
}

impl CoroutineSlot {
    pub fn checked(
        enums: &Arena<EnumDef>,
        value_payload: MirVariantFieldRef,
        empty: MirVariantRef,
        value: Type,
    ) -> Option<Self> {
        let value_variant = value_payload.variant();
        if value_variant.enum_id() != empty.enum_id()
            || value_variant.variant_index() == empty.variant_index()
        {
            return None;
        }
        let value_definition = value_variant.definition(enums).ok()?;
        let empty_definition = empty.definition(enums).ok()?;
        (enums[value_variant.enum_id()].type_arguments.is_empty()
            && enums[value_variant.enum_id()].variants.len() == 2
            && value_definition.fields.len() == 1
            && value_payload.field_index() == 0
            && value_payload.definition(enums).ok()?.ty == value
            && empty_definition.fields.is_empty())
        .then_some(Self {
            value_payload,
            empty,
            value,
        })
    }

    pub const fn enum_id(&self) -> EnumId {
        self.value_payload.variant().enum_id()
    }

    pub const fn value_variant(&self) -> MirVariantRef {
        self.value_payload.variant()
    }

    pub const fn value_payload(&self) -> MirVariantFieldRef {
        self.value_payload
    }

    pub const fn empty(&self) -> MirVariantRef {
        self.empty
    }

    pub const fn value(&self) -> &Type {
        &self.value
    }
}

/// A field in one exact compiler-generated coroutine frame class.
///
/// Binding the class and index in one value prevents a field role belonging
/// to one frame layout from being attached to another frame accidentally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoroutineFrameFieldRef {
    class: ClassId,
    field: u32,
}

impl CoroutineFrameFieldRef {
    pub fn checked(classes: &Arena<ClassDef>, class: ClassId, field: u32) -> Option<Self> {
        let definition = arena_get(classes, class)?;
        let ClassRepresentation::Declared { fields, .. } = &definition.representation else {
            return None;
        };
        fields.get(field as usize)?;
        Some(Self { class, field })
    }

    pub const fn class(self) -> ClassId {
        self.class
    }

    pub const fn field_index(self) -> u32 {
        self.field
    }

    pub fn definition(self, classes: &Arena<ClassDef>) -> Option<&Field> {
        let definition = arena_get(classes, self.class)?;
        let ClassRepresentation::Declared { fields, .. } = &definition.representation else {
            return None;
        };
        fields.get(self.field as usize)
    }
}

/// One exact typed value slot owned by a coroutine frame.
#[derive(Debug, Clone)]
pub struct CoroutineSavedValue {
    field: CoroutineFrameFieldRef,
    slot: CoroutineSlotId,
    value: Type,
}

impl CoroutineSavedValue {
    pub fn checked(
        classes: &Arena<ClassDef>,
        slots: &Arena<CoroutineSlot>,
        field: CoroutineFrameFieldRef,
        slot: CoroutineSlotId,
    ) -> Option<Self> {
        let metadata = arena_get(slots, slot)?;
        (field.definition(classes)?.ty == Type::Enum(metadata.enum_id(), Vec::new())).then(|| {
            Self {
                field,
                slot,
                value: metadata.value().clone(),
            }
        })
    }

    pub const fn field(&self) -> CoroutineFrameFieldRef {
        self.field
    }

    pub const fn slot(&self) -> CoroutineSlotId {
        self.slot
    }

    pub const fn value(&self) -> &Type {
        &self.value
    }
}

/// The exact `CoroutineSlot<Throwable>` consumed by failure resume entries.
/// It has a distinct id family from ordinary saved values, so a pending outer
/// throwable cannot alias the slot overwritten by an inner resume failure.
#[derive(Debug, Clone)]
pub struct CoroutineFailureValue {
    field: CoroutineFrameFieldRef,
    slot: CoroutineSlotId,
    throwable: ClassId,
}

impl CoroutineFailureValue {
    pub fn checked(
        classes: &Arena<ClassDef>,
        slots: &Arena<CoroutineSlot>,
        field: CoroutineFrameFieldRef,
        slot: CoroutineSlotId,
        throwable: ClassId,
    ) -> Option<Self> {
        arena_get(classes, throwable)?;
        let metadata = arena_get(slots, slot)?;
        (metadata.value() == &Type::Class(throwable)
            && field.definition(classes)?.ty == Type::Enum(metadata.enum_id(), Vec::new()))
        .then_some(Self {
            field,
            slot,
            throwable,
        })
    }

    pub const fn field(&self) -> CoroutineFrameFieldRef {
        self.field
    }

    pub const fn slot(&self) -> CoroutineSlotId {
        self.slot
    }

    pub const fn throwable(&self) -> ClassId {
        self.throwable
    }
}

#[derive(Debug)]
pub struct CoroutineFrame {
    class: ClassId,
    owner: CoroutineFunctionId,
    state: CoroutineFrameFieldRef,
    completion: CoroutineFrameFieldRef,
    saved_values: Vec<CoroutineSavedValueId>,
    failure: CoroutineFailureValueId,
}

impl CoroutineFrame {
    #[allow(clippy::too_many_arguments)]
    pub fn checked(
        classes: &Arena<ClassDef>,
        saved: &Arena<CoroutineSavedValue>,
        failures: &Arena<CoroutineFailureValue>,
        class: ClassId,
        owner: CoroutineFunctionId,
        state: CoroutineFrameFieldRef,
        completion: CoroutineFrameFieldRef,
        saved_values: Vec<CoroutineSavedValueId>,
        failure: CoroutineFailureValueId,
    ) -> Option<Self> {
        if state.class() != class
            || completion.class() != class
            || state.definition(classes)?.ty
                != Type::MachineScalar(MachineScalarKind::CoroutineFrameState)
        {
            return None;
        }
        let failure_metadata = arena_get(failures, failure)?;
        if failure_metadata.field().class() != class {
            return None;
        }
        let mut fields = vec![state.field_index(), completion.field_index()];
        for value in &saved_values {
            let metadata = arena_get(saved, *value)?;
            if metadata.field().class() != class {
                return None;
            }
            fields.push(metadata.field().field_index());
        }
        fields.push(failure_metadata.field().field_index());
        fields.sort_unstable();
        if fields.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }
        Some(Self {
            class,
            owner,
            state,
            completion,
            saved_values,
            failure,
        })
    }

    pub const fn class(&self) -> ClassId {
        self.class
    }

    pub const fn owner(&self) -> CoroutineFunctionId {
        self.owner
    }

    pub const fn state(&self) -> CoroutineFrameFieldRef {
        self.state
    }

    pub const fn completion(&self) -> CoroutineFrameFieldRef {
        self.completion
    }

    pub fn saved_values(&self) -> &[CoroutineSavedValueId] {
        &self.saved_values
    }

    pub const fn failure(&self) -> CoroutineFailureValueId {
        self.failure
    }

    pub fn owns_saved_value(&self, value: CoroutineSavedValueId) -> bool {
        self.saved_values.contains(&value)
    }
}

/// One static parent transfer retained while an inner cleanup suspends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutinePendingTransfer {
    Fallthrough(CoroutineCleanupFallthroughTarget),
    Return(CoroutineReturnTransfer),
    Break(CoroutineLoopExitTarget),
    Continue(CoroutineLoopHeaderTarget),
    ManagedThrow(CoroutineManagedThrowTransfer),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoroutineReturnTransfer {
    Unit,
    Saved(CoroutineSavedValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoroutineManagedThrowTransfer {
    exception: CoroutineSavedValueId,
    unwind: Option<CoroutineUnwindTarget>,
}

impl CoroutineManagedThrowTransfer {
    pub const fn new(
        exception: CoroutineSavedValueId,
        unwind: Option<CoroutineUnwindTarget>,
    ) -> Self {
        Self { exception, unwind }
    }

    pub const fn exception(self) -> CoroutineSavedValueId {
        self.exception
    }

    pub const fn unwind(self) -> Option<CoroutineUnwindTarget> {
        self.unwind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoroutineResumeSuccess {
    entry: CoroutineResumeEntryTarget,
    post: CoroutineCleanupFallthroughTarget,
}

impl CoroutineResumeSuccess {
    pub const fn new(
        entry: CoroutineResumeEntryTarget,
        post: CoroutineCleanupFallthroughTarget,
    ) -> Self {
        Self { entry, post }
    }

    pub const fn entry(self) -> CoroutineResumeEntryTarget {
        self.entry
    }

    pub const fn post(self) -> CoroutineCleanupFallthroughTarget {
        self.post
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoroutineResumeFailure {
    entry: CoroutineResumeEntryTarget,
    exception: CoroutineFailureValueId,
    unwind: Option<CoroutineUnwindTarget>,
}

impl CoroutineResumeFailure {
    pub const fn new(
        entry: CoroutineResumeEntryTarget,
        exception: CoroutineFailureValueId,
        unwind: Option<CoroutineUnwindTarget>,
    ) -> Self {
        Self {
            entry,
            exception,
            unwind,
        }
    }

    pub const fn entry(self) -> CoroutineResumeEntryTarget {
        self.entry
    }

    pub const fn exception(self) -> CoroutineFailureValueId {
        self.exception
    }

    pub const fn unwind(self) -> Option<CoroutineUnwindTarget> {
        self.unwind
    }
}

#[derive(Debug)]
pub struct CoroutineResumePoint {
    frame: CoroutineFrameId,
    site: CoroutineSuspendStateId,
    result: Type,
    adapter: ClassId,
    resume: FunctionId,
    resume_with_exception: FunctionId,
    parents: Vec<CoroutinePendingTransfer>,
    success: CoroutineResumeSuccess,
    failure: CoroutineResumeFailure,
}

impl CoroutineResumePoint {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: CoroutineFrameId,
        site: CoroutineSuspendStateId,
        result: Type,
        adapter: ClassId,
        resume: FunctionId,
        resume_with_exception: FunctionId,
        parents: Vec<CoroutinePendingTransfer>,
        success: CoroutineResumeSuccess,
        failure: CoroutineResumeFailure,
    ) -> Self {
        Self {
            frame,
            site,
            result,
            adapter,
            resume,
            resume_with_exception,
            parents,
            success,
            failure,
        }
    }

    pub const fn frame(&self) -> CoroutineFrameId {
        self.frame
    }

    pub const fn site(&self) -> CoroutineSuspendStateId {
        self.site
    }

    pub const fn success_state(&self) -> CoroutineFrameState {
        CoroutineFrameState::Suspended(self.site)
    }

    pub const fn failure_state(&self) -> CoroutineFrameState {
        CoroutineFrameState::ResumeFailure(self.site)
    }

    pub const fn result(&self) -> &Type {
        &self.result
    }

    pub const fn adapter(&self) -> ClassId {
        self.adapter
    }

    pub const fn resume(&self) -> FunctionId {
        self.resume
    }

    pub const fn resume_with_exception(&self) -> FunctionId {
        self.resume_with_exception
    }

    pub fn parents(&self) -> &[CoroutinePendingTransfer] {
        &self.parents
    }

    pub const fn success(&self) -> CoroutineResumeSuccess {
        self.success
    }

    pub const fn failure(&self) -> CoroutineResumeFailure {
        self.failure
    }
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
    Object(ObjectTypeId),
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
