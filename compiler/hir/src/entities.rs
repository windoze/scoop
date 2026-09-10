use super::*;

#[derive(Debug, Clone)]
pub struct Module {
    /// Total persistent identity relation for every nominal declaration in
    /// the five export arenas. Local arena ids remain request-scoped and are
    /// never used as emission or cross-Cone identity.
    pub nominal_identities: HirNominalIdentities,
    /// Total persistent identity relation for every source property. The
    /// identity kind distinguishes ordinary and extension declarations.
    pub property_identities: HirPropertyIdentities,
    /// Total identity and logical-property relation for both accessor arenas.
    pub property_accessor_identities: HirPropertyAccessorIdentities,
    /// Total persistent identity relation for transparent source aliases.
    pub type_alias_identities: HirTypeAliasIdentities,
    /// Total persistent identity relation for source enum variants and fields.
    pub enum_member_identities: HirEnumMemberIdentities,
    /// Total persistent identity relation for struct and class storage fields.
    pub field_identities: HirFieldIdentities,
    /// Total persistent identity and declaration relation for object values.
    pub object_value_identities: HirObjectValueIdentities,
    /// Total persistent identity relation for compiler-managed initialization
    /// units, including their typed declaration and storage ownership.
    pub initialization_unit_identities: HirInitializationUnitIdentities,
    /// Total classification and persistent identity relation for every HIR
    /// type. Open types retain their exact source binder set.
    pub type_identities: HirTypeIdentities,
    /// Total persistent identity relation for source constructors and the
    /// explicitly distinguished generated zero-argument adapters.
    pub constructor_identities: HirConstructorIdentities,
    /// Total persistent identity relation for source, accessor, lexical,
    /// initialization and compiler-derived functions.
    pub function_identities: HirFunctionIdentities,
    /// Direct public package bindings derived from the typed declaration
    /// surface. Member declarations remain reachable through their owners.
    pub export_binding_identities: HirExportBindingIdentities,
    /// Persistent slot identities for every class virtual family and direct
    /// interface member. Overrides keep the slot of their family root.
    pub dispatch_slot_identities: HirDispatchSlotIdentities,
    /// Total persistent identity relation for every typed source context.
    pub source_context_identities: HirSourceContextIdentities,
    /// Target-independent persistent contract for every source extern
    /// function and global.
    pub source_native_contracts: HirSourceNativeContracts,
    /// Explicit public source API. Internal/private implementation entities
    /// elsewhere in this module are not downstream declaration candidates.
    pub public_surface: PublicSemanticSurface,
    /// Driver-provided display names and source text indexed by every typed
    /// expression origin. Keeping this relation in Export HIR lets generic
    /// concretization consume source provenance without consulting the
    /// parser or filesystem again.
    pub source_files: Vec<SourceFileMetadata>,
    /// Request-local handles for source contexts referenced by expression
    /// origins. Their persistent identities live in the aligned relation.
    pub source_contexts: Arena<SourceContext>,
    pub types: Arena<Type>,
    /// Canonical function signatures in one-to-one correspondence with their
    /// `FunctionType::canonical_type` entries in `types`.
    pub function_types: Arena<FunctionType>,
    /// Source callable-value entities. Their identities are intentionally
    /// separate from the generated invoke functions they own.
    pub lambdas: Arena<Lambda>,
    pub anonymous_functions: Arena<AnonymousFunction>,
    pub local_functions: Arena<LocalFunction>,
    pub callable_references: Arena<CallableReference>,
    /// Template-only calls through an interface upper bound. Each entry
    /// names the exact receiver parameter, bound application and declaring
    /// interface method; local-concrete HIR has no corresponding arena.
    pub bound_callable_refs: Arena<BoundCallableRef>,
    /// Source/target signatures of every explicit function-value variance
    /// adaptation requested by HIR.
    pub function_coercions: Arena<FunctionCoercion>,
    pub foreign_callback_registrations: Arena<ForeignCallbackRegistration>,
    /// Source-call interfaces are export metadata. Concrete HIR consumes only
    /// the fully materialized runtime argument list.
    pub source_parameter_interfaces: Vec<ExportParameterInterface>,
    pub export_default_exprs: Arena<ExportDefaultExpr>,
    /// Declaration-view-specific relations from a default template's type
    /// parameters to the type parameters exposed by that source interface.
    pub export_default_sources: Arena<ExportDefaultSource>,
    pub export_vararg_parameter_types: Arena<ExportVarargParameterType>,
    pub functions: Arena<Function>,
    /// Native functions imported by source declarations. They have no HIR
    /// body and their identities never enter generic instantiation.
    pub extern_functions: Arena<ExternFunction>,
    /// Top-level storage declarations. Globals use an identity distinct from
    /// functions and locals, and every entry carries a complete storage kind.
    pub globals: Arena<Global>,
    pub initialization_units: Arena<InitializationUnit>,
    pub initialization_failure_roots: Arena<InitializationFailureRoot>,
    pub objects: Arena<ObjectDecl>,
    pub object_types: Arena<ObjectType>,
    pub companion_relations: Arena<CompanionRelation>,
    pub singleton_values: Arena<SingletonValue>,
    pub singleton_published_roots: Arena<SingletonPublishedRoot>,
    /// Logical properties and their independently typed accessor identities.
    /// Physical fields/globals are reachable only through a representation.
    pub properties: Arena<Property>,
    /// Top-level extension-property templates. Their identity is distinct
    /// from the logical property and from either generated accessor function.
    pub extension_properties: Arena<ExtensionProperty>,
    pub property_getters: Arena<PropertyGetter>,
    pub property_setters: Arena<PropertySetter>,
    /// Hidden effective-delegate storage, separate from both logical
    /// properties and the physical field/global identity it occupies.
    pub delegate_storages: Arena<DelegateStorage>,
    /// Top-level transparent aliases exported as source API. Alias identities
    /// are deliberately absent from `types` and LocalConcrete HIR.
    pub type_aliases: Arena<TypeAliasDecl>,
    /// Generic function definitions. Their ids are distinct from
    /// ordinary `FunctionId`s even though each entry points at the HIR
    /// function that owns the parameterized body.
    pub generic_functions: Arena<GenericFunction>,
    /// Exact ordinary method applications and non-virtual generic method
    /// templates/applications. Calls carry these typed identities directly;
    /// concretization never reconstructs an owner from a function or a flat
    /// argument vector.
    pub method_applications: Arena<MethodApplication>,
    pub generic_methods: Arena<GenericMethod>,
    pub generic_method_applications: Arena<GenericMethodApplication>,
    /// Fully typed compiler-derived equality bodies requested while lowering
    /// source operators. Each body already names every nested member/derived
    /// target; concretization substitutes it mechanically and never performs
    /// member lookup or reconstructs aggregate semantics.
    pub derived_equality_applications: Arena<DerivedEqualityApplication>,
    pub structs: Arena<StructDecl>,
    pub struct_constructors: Arena<StructConstructor>,
    pub struct_constructor_applications: Arena<StructConstructorApplication>,
    /// Canonical, fully applied export-side struct identities.  A type never
    /// stores a declaration id and an unrelated argument vector.
    pub struct_applications: Arena<StructApplication>,
    pub enums: Arena<EnumDecl>,
    pub enum_applications: Arena<EnumApplication>,
    pub classes: Arena<ClassDecl>,
    pub class_fields: Arena<ClassField>,
    pub class_constructors: Arena<ClassConstructor>,
    pub class_constructor_applications: Arena<ClassConstructorApplication>,
    pub class_applications: Arena<ClassApplication>,
    pub interfaces: Arena<InterfaceDecl>,
    pub interface_applications: Arena<InterfaceApplication>,
    /// Interface member declarations have their own identity domain. A
    /// bound call never uses a general `FunctionId` as a substitute for the
    /// declaring interface-member identity.
    pub interface_methods: Arena<InterfaceMethod>,
    /// Top-level functions in declaration order (core library first,
    /// then user code).
    pub top_level: Vec<FunctionId>,
    /// Non-integer well-known types allocated by HIR lowering. Canonical
    /// integer owners are held by `intrinsic_type_core.integers`; expression
    /// types carry their exact `IntegerKind` directly.
    pub unit: TypeId,
    pub boolean: TypeId,
    pub string: TypeId,
    /// The complete checked `Option` contract from `scoop.core` (the
    /// desugar target of `T?`, spec 7.1). Guaranteed present: a core library
    /// without the exact `Some(T)` / `None` shape is rejected before HIR.
    pub option_core: OptionCore,
    /// Compiler-owned source iteration protocol. Per-use exact applications
    /// and conformance witnesses remain in `ForIterationPlan`.
    pub iteration_core: IterationCore,
    /// Compiler-generated exception construction targets. Every entry is a
    /// validated, zero-argument class constructor; later stages never find
    /// these entities by source or link name.
    pub exception_core: CompilerExceptionCore,
    /// Compiler-known coroutine protocol entities. HIR lowering validates
    /// their exact declarations before constructing the module, so MIR never
    /// falls back to textual lookup for protocol types or methods.
    pub coroutine_core: CoroutineCore,
    /// Compiler-known pointer/FFI core entities. HIR lowering validates the
    /// unique source declarations and downstream stages use these typed ids,
    /// never textual names.
    pub ffi_core: FfiCore,
    /// Compiler-validated managed callback protocol. Its ids are export-side
    /// semantic identities and are concretized into a distinct local family.
    pub foreign_callback_core: ForeignCallbackCore,
    /// Source-validated nominal declarations for every compiler-represented
    /// core type. These typed ids are the only bridge from primitive/family
    /// semantics to source members and interfaces.
    pub intrinsic_type_core: IntrinsicTypeCore,
    /// Compiler-validated source-location value shape and its HIR intrinsic.
    pub source_location_core: SourceLocationCore,
    /// Resolved generic function applications, deduplicated in
    /// first-use order. The arena id is carried directly by call
    /// expressions and is the instantiation request consumed by MIR.
    pub instantiations: Arena<ResolvedGenericFunction>,
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
        reusable: AppliedEnumVariantRef,
        one_shot: AppliedEnumVariantRef,
    ) -> Option<Self> {
        for variant in [reusable, one_shot] {
            if AppliedEnumVariantRef::checked(
                enums,
                applications,
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
        let application = &applications[reusable.application()];
        let declaration = &enums[application.template];
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
        let application = &applications[registered.application()];
        let declaration = &enums[application.template];
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
        option: OptionCore,
        throwable: TypeId,
        some_payload: AppliedEnumVariantFieldRef,
        none: AppliedEnumVariantRef,
    ) -> Option<Self> {
        let some = some_payload.variant();
        if AppliedEnumVariantFieldRef::checked(
            enums,
            applications,
            some,
            some_payload.local_index(),
        ) != Some(some_payload)
            || AppliedEnumVariantRef::checked(
                enums,
                applications,
                none.application(),
                none.declaration(),
            ) != Some(none)
            || some.application() != none.application()
        {
            return None;
        }
        let application = &applications[some.application()];
        (application.template == option.enumeration()
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
    /// Typed root whose stable lexical traversal owns this conversion.
    pub definition_root: LexicalDefinitionRoot,
    /// Stable definition-site path of this callback conversion. Concrete
    /// instantiations preserve the path instead of allocating a new site.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub native_function_type: FunctionTypeId,
    pub managed_function_type: FunctionTypeId,
    pub context_index: u32,
    pub mode: AppliedEnumVariantRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackOperation {
    Retain,
    Release,
    State,
    Failure,
}

/// Typed root of one stable lexical definition traversal. Paths are complete
/// relative to this root, so nested callables keep the same root while
/// appending their own segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LexicalDefinitionRoot {
    Function(FunctionId),
    ClassConstructor(ClassConstructorId),
    StructConstructor(StructConstructorId),
    VariantConstructor(EnumVariantRef),
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub definition_root: LexicalDefinitionRoot,
    /// Stable lexical definition path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    /// Type parameters inherited from the enclosing generic callable. The
    /// generated invoke body is instantiated with this complete prefix.
    pub owner_type_param_count: usize,
    pub body_type_arguments: CallableBodyTypeArguments,
    /// Structurally present even for no-capture lambdas; later M11 capture
    /// analysis fills this list rather than changing the entity shape.
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AnonymousFunction {
    pub definition_root: LexicalDefinitionRoot,
    /// Stable lexical definition path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub owner_type_param_count: usize,
    pub body_type_arguments: CallableBodyTypeArguments,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CallableBodyTypeArguments {
    /// Substitute the type arguments of the ordinary enclosing body.
    Lexical,
    /// A hygienically expanded default fixes the generated body's original
    /// lexical parameters even though the expression now belongs to a
    /// different caller body.
    Explicit(Vec<TypeId>),
}

/// A block-local named function. `function` is its lifted body; direct calls
/// pass `captures` as hidden parameters, while taking `::name` materializes a
/// closure over the same body.
#[derive(Debug, Clone)]
pub struct LocalFunction {
    pub definition_root: LexicalDefinitionRoot,
    /// Stable lexical declaration path, independent of every arena id.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub function: FunctionId,
    pub function_type: FunctionTypeId,
    pub captures: Vec<Capture>,
    /// Type parameters inherited from enclosing generic callables form the
    /// prefix of the lifted function's combined type-parameter namespace.
    pub owner_type_param_count: usize,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallableReference {
    pub definition_root: LexicalDefinitionRoot,
    /// Stable definition-site path of the generated invoke wrapper.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub target: CallableReferenceTarget,
    pub function_type: FunctionTypeId,
    /// Type parameters of the callable containing this reference expression.
    /// A non-zero value requires a concrete closure per enclosing instance.
    pub owner_type_param_count: usize,
    pub captures: Vec<Capture>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CallableReferenceTarget {
    Named(Callable),
    Local {
        local_function: LocalFunctionId,
        callee: Callable,
    },
    /// A member reference whose receiver expression is evaluated when the
    /// closure is created. The receiver's static type remains attached to the
    /// expression so MIR can preserve direct / virtual / interface dispatch.
    BoundMember {
        receiver: Box<Expr>,
        callee: MethodCallee,
    },
    /// A bound extension reference. Unlike a member reference its invoke
    /// wrapper always direct-calls the extension body, prepending the saved
    /// receiver to the ordinary source arguments.
    BoundExtension {
        receiver: Box<Expr>,
        callee: Callable,
    },
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub binding: BindingId,
    pub name: String,
    pub ty: TypeId,
    pub first_use_span: Span,
    /// Expression evaluated in the immediately enclosing callable when the
    /// closure object is created. It is either a local read or a transitive
    /// capture read, and therefore preserves by-value creation-time semantics.
    pub source: Expr,
}

#[derive(Debug, Clone)]
pub struct FunctionCoercion {
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}

/// A constructor target whose zero-argument signature is guaranteed by its
/// type. The constructed exception type is exactly `class`; there is no
/// parallel return-type field that could disagree with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroArgClassConstructor {
    pub class: ClassId,
    pub constructor: ClassConstructorId,
}

/// A constructor whose single source/physical parameter is the exact core
/// `Option<String>` application used for compiler-generated messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageClassConstructor {
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
    pub throwable: CompilerException,
    pub unwrap_exception: CompilerException,
    pub class_cast_exception: CompilerException,
    pub arithmetic_exception: CompilerException,
    pub index_out_of_bounds_exception: CompilerException,
    pub illegal_state_exception: CompilerException,
    pub illegal_state_message_constructor: MessageClassConstructor,
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

impl Module {
    pub fn callable_function(&self, callable: impl FunctionCallee) -> FunctionId {
        callable.function(self)
    }
}

pub trait FunctionCallee: Copy {
    fn function(self, module: &Module) -> FunctionId;
}

impl FunctionCallee for Callable {
    fn function(self, module: &Module) -> FunctionId {
        callable_function(module, self)
    }
}

impl FunctionCallee for MethodCallee {
    fn function(self, module: &Module) -> FunctionId {
        method_callee_function(module, self)
    }
}

/// A generic type parameter that must denote a recursively GC-free value
/// whenever it is used as the pointee of the compiler-represented `Ptr`
/// family. This condition is intentionally distinct from a callable's
/// `@NoGC` effect requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequiresGcFreePointee {
    pub type_param: TypeParamId,
}

/// A generic HIR function definition. Generic identity is deliberately
/// separate from the underlying function identity (AGENTS.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericFunction {
    pub function: FunctionId,
    /// Type parameters whose concrete arguments must be GC-free for this
    /// generic definition to satisfy its `@NoGC` contract. The requirement
    /// is inferred from the resolved signature/body and checked at every
    /// instantiation; unused/representation-erased parameters are omitted.
    pub no_gc_type_params: Vec<TypeParamId>,
    /// Pointee constraints inferred from this template's signature/body and
    /// transitively required generic calls.
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
}

/// A generic function with every call-site type argument resolved.
/// MIR consumes this entity to produce a separate monomorphized
/// function entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGenericFunction {
    pub generic: GenericFunctionId,
    pub type_args: Vec<TypeId>,
}

/// Exact source-side owner of a method call. Parameter-free owners still use
/// their canonical empty application, so every method application has one
/// uniform and complete representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodOwnerApplication {
    Class(ClassApplicationId),
    Struct(StructApplicationId),
    Enum(EnumApplicationId),
    Interface(InterfaceApplicationId),
    Object(ObjectTypeId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodApplication {
    pub function: FunctionId,
    pub owner: MethodOwnerApplication,
}

/// Exact origin of one compiler-derived value-type equality method.
/// Nominal declarations reuse their owner application; structural Unit/tuple
/// methods carry their owner type directly because they have no declaration
/// arena whose identity could stand in for that type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DerivedEqualityOrigin {
    Nominal(MethodOwnerApplication),
    Structural(TypeId),
}

/// One application of a compiler-derived value-type equality method. The
/// source method declaration supplies stable callable identity; every other
/// property needed to materialize the concrete method is mandatory here.
/// `body` is complete application-specific HIR, including exact nested
/// callees, so concretization only substitutes types and callable identities.
#[derive(Debug, Clone)]
pub struct DerivedEqualityApplication {
    pub function: FunctionId,
    pub origin: DerivedEqualityOrigin,
    pub owner_ty: TypeId,
    pub attributes: FunctionAttributes,
    pub span: Span,
    pub body: Body,
}

/// A non-virtual generic method template. Its declaration identity is
/// separate from every other generic callable family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericMethod {
    pub function: FunctionId,
    pub no_gc_type_params: Vec<TypeParamId>,
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
}

/// Exact owner kinds accepted by non-interface generic methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenericMethodOwner {
    Class(ClassApplicationId),
    Struct(StructApplicationId),
    Enum(EnumApplicationId),
    Object(ObjectTypeId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenericMethodApplication {
    pub method: GenericMethodId,
    pub owner: GenericMethodOwner,
    pub method_arguments: NonEmptyVec<TypeId>,
}

/// The fully-resolved callable stored on HIR calls. A generic call
/// cannot be represented as a plain function plus an unrelated type
/// argument vector: it must reference a resolved generic entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callable {
    Function(FunctionId),
    Generic(ResolvedGenericFunctionId),
    Method(MethodApplicationId),
    GenericMethod(GenericMethodApplicationId),
}

pub(crate) fn callable_function(module: &Module, callable: Callable) -> FunctionId {
    match callable {
        Callable::Function(function) => function,
        Callable::Generic(id) => {
            module.generic_functions[module.instantiations[id].generic].function
        }
        Callable::Method(id) => module.method_applications[id].function,
        Callable::GenericMethod(id) => {
            module.generic_methods[module.generic_method_applications[id].method].function
        }
    }
}

pub(crate) fn method_callee_function(module: &Module, callee: MethodCallee) -> FunctionId {
    match callee {
        MethodCallee::Callable(callable) => callable_function(module, callable),
        MethodCallee::Bound(bound) => match module.bound_callable_refs[bound].source {
            BoundCallableSource::Class { callable, .. } => callable_function(module, callable),
            BoundCallableSource::Interface { member, .. } => {
                module.interface_methods[member].function
            }
        },
        MethodCallee::DerivedEquality(application) => {
            module.derived_equality_applications[application].function
        }
    }
}
