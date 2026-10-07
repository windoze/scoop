use super::*;

/// Closed compiler-protocol authority carried by one Export HIR graph.
///
/// Keeping the variants disjoint prevents ordinary lowering from presenting
/// imported persistent subjects as declarations owned by the current Cone.
#[derive(Debug, Clone)]
pub enum CoreProtocols {
    Defined(Box<DefinedCoreProtocols>),
    Imported(std::sync::Arc<ImportedCoreProtocols>),
}

/// Complete compiler-facing protocol authority defined by the current HIR
/// declaration graph. Every constituent is mandatory and is validated before
/// a `Module` can be returned.
#[derive(Debug, Clone)]
pub struct DefinedCoreProtocols {
    /// Checked `Option<T>` source contract used by nullable syntax.
    pub option: OptionCore,
    /// Source iteration protocol and its dispatch target.
    pub iteration: IterationCore,
    /// Compiler-generated exception construction targets.
    pub exceptions: CompilerExceptionCore,
    /// Coroutine protocol declarations and intrinsic operations.
    pub coroutines: CoroutineCore,
    /// Pointer and FFI declarations and operations.
    pub ffi: FfiCore,
    /// Managed foreign-callback protocol.
    pub foreign_callbacks: ForeignCallbackCore,
    /// Source owners of compiler-represented fundamental types.
    pub fundamental_types: IntrinsicTypeCore,
    /// Source-location value shape and intrinsic operation.
    pub source_location: SourceLocationCore,
}

#[derive(Debug, Clone, Copy)]
pub struct FfiCore {
    pub ptr: StructId,
    pub fun_ptr: StructId,
    pub pinned_ptr: StructId,
    pub gc_handle: StructId,
    pub ptr_to_ulong: FunctionId,
    pub ptr_cast: FunctionId,
    pub ptr_load: FunctionId,
    pub ptr_load_offset: FunctionId,
    pub ptr_store: FunctionId,
    pub ptr_store_offset: FunctionId,
    pub ptr_plus: FunctionId,
    pub ptr_minus: FunctionId,
    pub address_of: FunctionId,
    pub size_of: FunctionId,
    pub align_of: FunctionId,
    pub gc_pin_raw: FunctionId,
    pub gc_unpin_raw: FunctionId,
    pub gc_get_handle_raw: FunctionId,
    pub gc_release_handle_raw: FunctionId,
}

#[derive(Debug, Clone, Copy)]
pub struct ForeignCallbackCore {
    pub callback: StructId,
    pub modes: ForeignCallbackModes,
    pub states: ForeignCallbackStates,
    pub failure_result: ForeignCallbackFailureResult,
    pub register: FunctionId,
    pub retain: FunctionId,
    pub release: FunctionId,
    pub query_state: FunctionId,
    pub failure: FunctionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackModes {
    reusable: AppliedEnumVariantRef,
    one_shot: AppliedEnumVariantRef,
}

impl ForeignCallbackModes {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        nominal_identities: &HirNominalIdentities,
        reusable: AppliedEnumVariantRef,
        one_shot: AppliedEnumVariantRef,
    ) -> Option<Self> {
        for variant in [reusable, one_shot] {
            if AppliedEnumVariantRef::checked(
                enums,
                applications,
                nominal_identities,
                variant.application(),
                variant.declaration(),
            ) != Some(variant)
            {
                return None;
            }
        }
        if reusable.application() != one_shot.application() {
            return None;
        }
        let declaration = &enums[reusable.declaration().enumeration()];
        let reusable_definition = declaration.variants.get(reusable.local_index() as usize)?;
        let one_shot_definition = declaration.variants.get(one_shot.local_index() as usize)?;
        (declaration.name == "ForeignCallbackMode"
            && declaration.self_application == reusable.application()
            && declaration.type_params.is_empty()
            && declaration.interfaces.is_empty()
            && declaration.variants.len() == 2
            && reusable != one_shot
            && reusable.local_index() == 0
            && one_shot.local_index() == 1
            && reusable_definition.name == "Reusable"
            && reusable_definition.fields.is_empty()
            && one_shot_definition.name == "OneShot"
            && one_shot_definition.fields.is_empty())
        .then_some(Self { reusable, one_shot })
    }

    pub const fn reusable(self) -> AppliedEnumVariantRef {
        self.reusable
    }

    pub const fn one_shot(self) -> AppliedEnumVariantRef {
        self.one_shot
    }

    pub const fn application(self) -> EnumApplicationId {
        self.reusable.application()
    }

    pub const fn enumeration(self) -> EnumId {
        self.reusable.declaration().enumeration()
    }

    pub fn contains(self, variant: AppliedEnumVariantRef) -> bool {
        variant == self.reusable || variant == self.one_shot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackStates {
    registered: AppliedEnumVariantRef,
    active: AppliedEnumVariantRef,
    completed: AppliedEnumVariantRef,
    failed: AppliedEnumVariantRef,
}

impl ForeignCallbackStates {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        nominal_identities: &HirNominalIdentities,
        registered: AppliedEnumVariantRef,
        active: AppliedEnumVariantRef,
        completed: AppliedEnumVariantRef,
        failed: AppliedEnumVariantRef,
    ) -> Option<Self> {
        let variants = [registered, active, completed, failed];
        for variant in variants {
            if AppliedEnumVariantRef::checked(
                enums,
                applications,
                nominal_identities,
                variant.application(),
                variant.declaration(),
            ) != Some(variant)
            {
                return None;
            }
        }
        if variants
            .iter()
            .any(|variant| variant.application() != registered.application())
        {
            return None;
        }
        let declaration = &enums[registered.declaration().enumeration()];
        let expected = [
            (registered, "Registered"),
            (active, "Active"),
            (completed, "Completed"),
            (failed, "Failed"),
        ];
        (declaration.name == "ForeignCallbackState"
            && declaration.self_application == registered.application()
            && declaration.type_params.is_empty()
            && declaration.interfaces.is_empty()
            && declaration.variants.len() == 4
            && expected.iter().enumerate().all(|(index, (variant, name))| {
                variant.local_index() as usize == index
                    && declaration.variants[index].name == *name
                    && declaration.variants[index].fields.is_empty()
            }))
        .then_some(Self {
            registered,
            active,
            completed,
            failed,
        })
    }

    pub const fn registered(self) -> AppliedEnumVariantRef {
        self.registered
    }

    pub const fn active(self) -> AppliedEnumVariantRef {
        self.active
    }

    pub const fn completed(self) -> AppliedEnumVariantRef {
        self.completed
    }

    pub const fn failed(self) -> AppliedEnumVariantRef {
        self.failed
    }

    pub const fn application(self) -> EnumApplicationId {
        self.registered.application()
    }

    pub const fn enumeration(self) -> EnumId {
        self.registered.declaration().enumeration()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackFailureResult {
    some_payload: AppliedEnumVariantFieldRef,
    none: AppliedEnumVariantRef,
}

impl ForeignCallbackFailureResult {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        nominal_identities: &HirNominalIdentities,
        option: OptionCore,
        throwable: TypeId,
        some_payload: AppliedEnumVariantFieldRef,
        none: AppliedEnumVariantRef,
    ) -> Option<Self> {
        let some = some_payload.variant();
        if AppliedEnumVariantFieldRef::checked(
            enums,
            applications,
            nominal_identities,
            some,
            some_payload.local_index(),
        ) != Some(some_payload)
            || AppliedEnumVariantRef::checked(
                enums,
                applications,
                nominal_identities,
                none.application(),
                none.declaration(),
            ) != Some(none)
            || some.application() != none.application()
        {
            return None;
        }
        let application = &applications[some.application()];
        (some.declaration().enumeration() == option.enumeration()
            && application.arguments.as_slice() == [throwable]
            && some.declaration() == option.some()
            && some_payload.local_index() == option.some_payload().local_index()
            && none.declaration() == option.none())
        .then_some(Self { some_payload, none })
    }

    pub const fn application(self) -> EnumApplicationId {
        self.some_payload.variant().application()
    }

    pub const fn some_payload(self) -> AppliedEnumVariantFieldRef {
        self.some_payload
    }

    pub const fn none(self) -> AppliedEnumVariantRef {
        self.none
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IntrinsicTypeCore {
    pub any: ClassId,
    pub nothing: ClassId,
    pub unit: StructId,
    pub character: StructId,
    pub float: StructId,
    pub double: StructId,
    pub integers: IntegerTypeCore<StructId>,
    pub boolean: StructId,
    pub string: ClassId,
    pub array: ClassId,
    pub mutable_array: ClassId,
    pub ptr: StructId,
    pub fun_ptr: StructId,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceLocationCore {
    pub location: StructId,
    pub current: FunctionId,
}

#[derive(Debug, Clone)]
pub struct ForeignCallbackRegistration {
    pub definition: ForeignCallbackDefinition,
    pub native_function_type: FunctionTypeId,
    pub managed_function_type: FunctionTypeId,
    pub context_index: u32,
    pub mode: scoop_identity::CallbackMode,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ForeignCallbackDefinition {
    Source {
        root: LexicalDefinitionRoot,
        path: scoop_identity::StructuralDefinitionPath,
    },
    Imported {
        identity: Box<crate::HirCallbackRegistrationIdentity>,
        definition_origin: Box<scoop_identity::DefinitionOrigin>,
        origin: crate::DefinitionOrigin,
        arguments: Vec<TypeId>,
    },
}

impl ForeignCallbackDefinition {
    pub fn path(&self) -> &scoop_identity::StructuralDefinitionPath {
        match self {
            Self::Source { path, .. } => path,
            Self::Imported { identity, .. } => identity.key().path(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackOperation {
    Retain,
    Release,
    State,
    Failure,
}

/// A constructor target whose zero-argument signature is guaranteed by its
/// type. The constructed exception type is exactly `class`; there is no
/// parallel return-type field that could disagree with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroArgClassConstructor {
    pub class: ClassId,
    pub constructor: ClassConstructorId,
}

/// One compiler-known exception type together with its only construction
/// target needed by compiler-generated control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerException {
    pub constructor: ZeroArgClassConstructor,
}

impl CompilerException {
    pub const fn class(self) -> ClassId {
        self.constructor.class
    }

    pub const fn callable(self) -> ClassConstructorId {
        self.constructor.constructor
    }
}

/// Complete exception capabilities emitted by Export HIR. These ids belong
/// exclusively to the export-side family and are concretized before MIR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerExceptionCore {
    pub missing_context_constructor: ClassConstructorId,
    pub throwable: CompilerException,
    pub unwrap_exception: CompilerException,
    pub class_cast_exception: CompilerException,
    pub arithmetic_exception: CompilerException,
    pub index_out_of_bounds_exception: CompilerException,
    pub illegal_argument_exception: CompilerException,
    pub illegal_state_exception: CompilerException,
    /// Core-internal `(String) -> Unit` service that constructs and throws
    /// the cycle `IllegalStateException` without exporting `Option<String>`.
    pub initialization_cycle_thrower: FunctionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoroutineCore {
    pub continuation: InterfaceId,
    pub continuation_resume: FunctionId,
    pub continuation_resume_with_exception: FunctionId,
    pub suspend_task: InterfaceId,
    pub suspend_task_run: FunctionId,
    pub suspend_registration: InterfaceId,
    pub suspend_registration_register: FunctionId,
    pub start_coroutine: FunctionId,
    pub suspend_coroutine: FunctionId,
}
