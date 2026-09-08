//! Export surface split (DESIGN 4.3): the wire-role decomposition of the
//! Export HIR. Each surface lists its typed roots; an entity referenced
//! by several closures still has one definition, and hidden support or
//! inheritance-only entities carry no binding-index entry. Local
//! compilation continues to consume the whole module; the T19 packager
//! serializes from these roots.

use std::collections::HashMap;

use crate::declarations::EnumVariantRef;
use crate::ids::PackageId;
use crate::ids::*;
use crate::imports::ReExport;

/// One entity addressable by the export surfaces. Session-local typed
/// ids; the wire form carries persistent ids and is produced by the
/// packager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportEntity {
    Function(FunctionId),
    Property(PropertyId),
    PropertyGetter(PropertyGetterId),
    PropertySetter(PropertySetterId),
    GenericFunction(GenericFunctionId),
    GenericMethod(GenericMethodId),
    Struct(StructId),
    StructConstructor(StructConstructorId),
    Enum(EnumId),
    Class(ClassId),
    ClassConstructor(ClassConstructorId),
    Interface(InterfaceId),
    InterfaceMethod(InterfaceMethodId),
    Object(ObjectId),
    ObjectType(ObjectTypeId),
    CompanionRelation(CompanionRelationId),
    SingletonValue(SingletonValueId),
    TypeAlias(ExportTypeAliasId),
    /// One enum variant published by a re-export binding.
    Variant(EnumVariantRef),
}

/// DESIGN 4.3 surface 1: source-explicit public declarations whose
/// effective lookup domain is universal, public aliases, and the
/// resolved re-export bindings.
#[derive(Debug, Clone, Default)]
pub struct PublicLookupSurface {
    pub functions: Vec<FunctionId>,
    pub properties: Vec<PropertyId>,
    pub property_getters: Vec<PropertyGetterId>,
    pub property_setters: Vec<PropertySetterId>,
    pub generic_functions: Vec<GenericFunctionId>,
    pub generic_methods: Vec<GenericMethodId>,
    pub structs: Vec<StructId>,
    pub struct_constructors: Vec<StructConstructorId>,
    pub enums: Vec<EnumId>,
    pub classes: Vec<ClassId>,
    pub class_constructors: Vec<ClassConstructorId>,
    pub interfaces: Vec<InterfaceId>,
    pub interface_methods: Vec<InterfaceMethodId>,
    pub objects: Vec<ObjectId>,
    pub object_types: Vec<ObjectTypeId>,
    pub companion_relations: Vec<CompanionRelationId>,
    pub singleton_values: Vec<SingletonValueId>,
    pub type_aliases: Vec<ExportTypeAliasId>,
    /// Published re-export bindings, in semantic-surface order.
    pub reexports: Vec<ReExport>,
}

/// DESIGN 4.3 surface 2: the protected inheritance surface of publicly
/// inheritable owners (public open/abstract classes and public
/// interfaces). Protected constructors and members enter here only;
/// without a subclass witness they have no ordinary binding entry.
#[derive(Debug, Clone, Default)]
pub struct InheritanceSurface {
    /// Publicly inheritable classes (open/abstract).
    pub open_classes: Vec<ClassId>,
    /// Public interfaces (all are inheritable contracts).
    pub interfaces: Vec<InterfaceId>,
    /// Protected constructors of the open classes.
    pub protected_class_constructors: Vec<(ClassId, ClassConstructorId)>,
    /// Protected member functions of the surface owners.
    pub protected_methods: Vec<(InheritanceHost, FunctionId)>,
    /// Protected member properties of the surface owners.
    pub protected_properties: Vec<(InheritanceHost, PropertyId)>,
    /// Interface slots declared by surface interfaces.
    pub interface_methods: Vec<(InterfaceId, InterfaceMethodId)>,
}

/// One owner of a protected member on the inheritance surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InheritanceHost {
    Class(ClassId),
    Interface(InterfaceId),
}

/// DESIGN 4.3 surface 3: public generic templates plus the
/// template-owned bodies and concretization predicates that only exist
/// to be substituted in consumers.
#[derive(Debug, Clone, Default)]
pub struct TemplateSupportClosure {
    pub generic_structs: Vec<StructId>,
    pub generic_enums: Vec<EnumId>,
    pub generic_classes: Vec<ClassId>,
    pub generic_interfaces: Vec<InterfaceId>,
    pub generic_functions: Vec<GenericFunctionId>,
    pub generic_methods: Vec<GenericMethodId>,
}

/// DESIGN 4.3 surface 4: the nominal/interface/alias/const/default
/// entities referenced by the signatures of the other surfaces,
/// transitively through the type graph.
#[derive(Debug, Clone, Default)]
pub struct InterfaceDependencyClosure {
    pub structs: Vec<StructId>,
    pub enums: Vec<EnumId>,
    pub classes: Vec<ClassId>,
    pub interfaces: Vec<InterfaceId>,
    pub objects: Vec<ObjectId>,
    /// Public `const val` properties reachable from surface signatures.
    pub const_properties: Vec<PropertyId>,
}

/// DESIGN 4.3 surface 5: M17 source-parameter interfaces and their
/// default templates (which only reference refined export-interface
/// refs).
#[derive(Debug, Clone, Default)]
pub struct SourceInterfaceTemplates {
    /// Indices into `Module::source_parameter_interfaces`.
    pub interfaces: Vec<usize>,
    /// Default-expression arenas entries owned by public callables.
    pub default_exprs: Vec<ExportDefaultExprId>,
}

/// DESIGN 4.3 surface 6: package/name/namespace to typed roots with
/// provenance. Hidden support and inheritance-only entities have no
/// entry; the reader API cannot enumerate their short names.
#[derive(Debug, Clone, Default)]
pub struct BindingIndex {
    /// Sorted by (package, name, namespace); one entry per key.
    pub entries: Vec<BindingEntry>,
}

impl BindingIndex {
    pub fn entry(
        &self,
        package: PackageId,
        name: &str,
        namespace: BindingNamespace,
    ) -> Option<&BindingEntry> {
        self.entries.iter().find(|entry| {
            entry.package == package && entry.name == name && entry.namespace == namespace
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingEntry {
    pub package: PackageId,
    pub name: String,
    pub namespace: BindingNamespace,
    /// Non-empty at construction; overload sets share one entry.
    pub roots: Vec<ExportEntity>,
    pub provenance: BindingProvenance,
}

/// The two ordinary binding namespaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BindingNamespace {
    Value,
    Type,
}

/// Why this Cone publishes the binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingProvenance {
    /// A public declaration of this Cone.
    PublicDeclaration,
    /// A validated `public import` re-export binding.
    ReExport,
}

/// The six DESIGN 4.3 surfaces, assembled once per compilation.
#[derive(Debug, Clone, Default)]
pub struct ExportSurfaces {
    pub public_lookup: PublicLookupSurface,
    pub inheritance: InheritanceSurface,
    pub template_support: TemplateSupportClosure,
    pub interface_dependency: InterfaceDependencyClosure,
    pub source_interface_templates: SourceInterfaceTemplates,
    pub binding_index: BindingIndex,
    /// The union of wire purposes per referenced entity. Definitions are
    /// single; this set is what the wire definition carries.
    pub purposes: HashMap<ExportEntity, ExportPurposes>,
}

/// The closed set of wire purposes one entity definition can carry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExportPurposes {
    pub public_lookup: bool,
    pub inheritance: bool,
    pub template_support: bool,
    pub interface_dependency: bool,
    pub source_interface_template: bool,
}
