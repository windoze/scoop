use super::*;

mod callables;
mod core_protocols;
mod lexical_callables;
pub use callables::*;
pub use core_protocols::*;
pub use lexical_callables::*;

#[derive(Debug, Clone)]
pub struct Module {
    /// Cone whose declarations and generated definitions this module emits.
    /// Imported/core sources may also be present, so this cannot be inferred
    /// from the source table.
    pub cone: scoop_identity::ConeIdentity,
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
    /// Total persistent identity relation for source foreign-callback
    /// conversion sites.
    pub callback_registration_identities: HirCallbackRegistrationIdentities,
    /// Direct public package bindings derived from the typed declaration
    /// surface plus public re-export bindings. Member declarations remain
    /// reachable through their owners.
    pub export_binding_identities: HirExportBindingIdentities,
    /// Canonical source classification for every public export binding.
    /// Declared-current entries form the legacy direct-public inventory;
    /// re-exports retain their complete validated route set.
    pub public_export_bindings: CanonicalPublicExportBindingsV1,
    /// Persistent current-Cone declaration/import bindings consumed by the
    /// identity foundation. These ids never replace request-local body
    /// `BindingId` values.
    pub local_binding_identities: HirLocalBindingIdentities,
    /// Persistent slot identities for every class virtual family and direct
    /// interface member. Overrides keep the slot of their family root.
    pub dispatch_slot_identities: HirDispatchSlotIdentities,
    /// Total persistent identity relation for every typed source context.
    pub source_context_identities: HirSourceContextIdentities,
    /// Target-independent persistent contract for every source extern
    /// function and global.
    pub source_native_contracts: HirSourceNativeContracts,
    /// Canonical foundation origins for every persistent subject established
    /// before exact callable materialization. Local-value origins are derived
    /// alongside LocalConcrete HIR and therefore live outside this relation.
    pub export_definition_origins: HirExportDefinitionOrigins,
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
    /// Imported core call targets selected for this ordinary HIR graph. Each
    /// entry retains the brand of the selected-set world that admitted it;
    /// expressions reference this arena instead of a core declaration id.
    pub imported_core_callables: Arena<ImportedCoreCallableUse>,
    /// Ordinary-dependency callables committed by this HIR graph. These uses
    /// have a separate id domain from trusted-core and local callables.
    pub imported_dependency_callables: Arena<ImportedDependencyCallableUse>,
    /// Imported core type targets selected for this ordinary HIR graph. The
    /// arena is independent from callable and value uses, so their local ids
    /// cannot be interchanged.
    pub imported_core_types: Arena<ImportedCoreTypeUse>,
    /// Imported core value targets selected for this ordinary HIR graph.
    pub imported_core_values: Arena<ImportedCoreValueUse>,
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
    pub default_local_value_scopes: Arena<DefaultLocalValueScope>,
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
    /// The single compiler-protocol authority for this HIR graph.
    ///
    /// Bootstrap graphs define the complete protocol product locally;
    /// ordinary graphs import the complete product from their trusted core
    /// artifact. The two worlds are deliberately mutually exclusive.
    pub core_protocols: CoreProtocols,
    /// Resolved generic function applications, deduplicated in
    /// first-use order. The arena id is carried directly by call
    /// expressions and is the instantiation request consumed by MIR.
    pub instantiations: Arena<ResolvedGenericFunction>,
}

impl Module {
    pub fn callable_function(&self, callable: impl FunctionCallee) -> FunctionId {
        callable.function(self)
    }
}
