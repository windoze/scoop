use super::*;

mod references;
use crate::HirNominalIdentity;
pub use references::*;
use scoop_identity::{PersistentEnumVariantFieldId, PersistentEnumVariantId};

/// Static declaration owner retained after concretization. Origins name the
/// declaration template rather than an outer application, so a generic host
/// does not replicate its nested declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NominalOwner {
    Class(HirNominalIdentity),
    Interface(HirNominalIdentity),
    Struct(HirNominalIdentity),
    Enum(HirNominalIdentity),
    Object(HirNominalIdentity),
}

#[derive(Debug, Clone)]
pub struct ObjectDecl {
    pub origin: HirNominalIdentity,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub object_type: ObjectTypeId,
    pub singleton_value: SingletonValueId,
    pub kind: ObjectKind,
    pub backing_class: ClassId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Standalone,
    Companion(CompanionRelationId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionRelation {
    pub host: NominalOwner,
    pub object: ObjectId,
    pub name: CompanionName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompanionName {
    Default,
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectType {
    pub declaration: ObjectId,
    pub representation: ClassId,
    pub canonical_type: TypeId,
}

/// A selected singleton value retains this IR's declaration handle or its
/// original dependency identity. Initialization and storage stay with that owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingletonValueTarget {
    Local(SingletonValueId),
    Dependency(scoop_identity::PersistentObjectValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingletonValue {
    pub identity: scoop_identity::PersistentObjectValueId,
    pub declaration: ObjectId,
    pub object_type: ObjectTypeId,
    pub published_root: SingletonPublishedRootId,
    pub initialization: InitializationUnitId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingletonPublishedRoot {
    pub value: SingletonValueId,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub origin: HirNominalIdentity,
    /// Canonical concrete type represented by this physical declaration.
    pub canonical_type: TypeId,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub type_arguments: Vec<TypeId>,
    pub gc_free: bool,
    pub representation: StructRepresentation,
    /// Direct source conformance after substituting this nominal's arguments.
    pub direct_interfaces: Vec<TypeId>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StructRepresentation {
    Declared(DeclaredStruct),
    Intrinsic {
        declaration: IntrinsicTypeKind,
        application: IntrinsicTypeRepresentation,
    },
}

#[derive(Debug, Clone)]
pub struct DeclaredStruct {
    pub attributes: StructAttributes,
    pub c_abi: StructCAbi,
    pub fields: Vec<DeclaredStructField>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructCAbi {
    SourceRepresentation,
    UInt64Field {
        field: scoop_identity::PersistentFieldId,
    },
}

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub representation: crate::EnumRepresentation,
    pub origin: HirNominalIdentity,
    /// Canonical concrete type represented by this exact enum application.
    pub canonical_type: TypeId,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub type_arguments: Vec<TypeId>,
    pub gc_free: bool,
    pub variants: Vec<Variant>,
    /// Direct source conformance after substituting this nominal's arguments.
    pub direct_interfaces: Vec<TypeId>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub release_policy: ReleasePolicy<ReleaseHookTarget>,
    pub origin: HirNominalIdentity,
    /// Canonical concrete type represented by this physical declaration.
    pub canonical_type: TypeId,
    pub modifier: ClassModifier,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub type_arguments: Vec<TypeId>,
    pub representation: ClassRepresentation,
    /// Direct source conformance, excluding inherited implementations.
    pub direct_interfaces: Vec<TypeId>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<ClassMethod>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub enum ClassMethod {
    Local(FunctionId),
    Imported {
        family: VirtualMethodId,
        callable: ImportedDependencyCallableUseId,
    },
}

/// Complete signature and owning class of one compiler-hidden constructor.
/// This is a callable entity in LocalConcreteHir, not a request for MIR to
/// discover or synthesize a target from the class name or id.
#[derive(Debug, Clone)]
pub struct ClassConstructor {
    pub class: ClassId,
    pub safety: Safety,
    /// Persistent identity of this exact constructor implementation. Generic
    /// nominal owners use their constructor application as the context;
    /// parameter-free constructors use `NoSubstitution`.
    pub materialization: CallableMaterialization,
    pub source_discriminator: u32,
    pub origin: DefinitionOrigin,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: ClassConstructorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassInitializerTarget {
    Local(ClassConstructorId),
    Imported(ImportedDependencyCallableUseId),
}

#[derive(Debug, Clone)]
pub struct ConstructorParameter {
    pub selector: scoop_identity::LocalValueSelector,
    pub id: ConstructorParamId,
    pub binding: BindingId,
    pub definition: DefinitionOrigin,
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub enum ClassConstructorKind {
    This {
        target: ClassConstructorId,
        body: Body,
    },
    Terminal {
        body: Body,
    },
}

impl ClassConstructor {
    pub fn body(&self) -> &Body {
        match &self.kind {
            ClassConstructorKind::This { body, .. } | ClassConstructorKind::Terminal { body } => {
                body
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructConstructor {
    pub structure: StructId,
    pub safety: Safety,
    /// Persistent identity of this exact constructor implementation.
    pub materialization: CallableMaterialization,
    pub source_discriminator: u32,
    pub origin: DefinitionOrigin,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: StructConstructorKind,
}

#[derive(Debug, Clone)]
pub enum StructConstructorKind {
    Primary,
    Secondary {
        gc_effect: GcEffect,
        target: StructConstructorId,
        arguments: ConstructorArguments,
        body: Body,
    },
}

#[derive(Debug, Clone)]
pub struct ConstructorArguments {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub enum ClassRepresentation {
    Declared(DeclaredClass),
    Intrinsic {
        declaration: IntrinsicTypeKind,
        application: IntrinsicTypeRepresentation,
    },
}

#[derive(Debug, Clone)]
pub struct DeclaredClass {
    pub fields: Vec<Field>,
    pub base_class: Option<ClassId>,
}

impl ClassDef {
    pub fn base_class(&self) -> Option<ClassId> {
        match &self.representation {
            ClassRepresentation::Declared(DeclaredClass { base_class, .. }) => *base_class,
            ClassRepresentation::Intrinsic { .. } => None,
        }
    }
}

/// Complete concrete representation selected by an intrinsic declaration and
/// this application's already-lowered arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    MaybeUninit { value: TypeId },
    Atomic(scoop_identity::AtomicStorage<TypeId>),
    Unit,
    Integer(IntegerKind),
    Boolean,
    Char,
    Float(crate::FloatKind),
    String,
    Array { element: TypeId },
    MutableArray { element: TypeId },
    Ptr { pointee: TypeId },
    FunPtr { signature: FunctionTypeId },
    Any,
    Nothing,
}

#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub origin: HirNominalIdentity,
    /// Canonical concrete type represented by this exact interface application.
    pub canonical_type: TypeId,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub family: InterfaceFamilyId,
    pub type_arguments: Vec<TypeId>,
    pub parents: Vec<TypeId>,
    pub methods: Vec<MethodSig>,
    pub span: Span,
}

/// Complete local-concrete dispatch table for one exact interface
/// application. Entries are paired with their typed slot identities.
#[derive(Debug, Clone)]
pub struct InterfaceImplementation {
    pub interface: InterfaceId,
    pub methods: Vec<InterfaceMethodImplementation>,
}

#[derive(Debug, Clone)]
pub struct InterfaceMethodImplementation {
    pub slot: InterfaceMethodSlot,
    pub target: InterfaceImplementationTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum InterfaceImplementationTarget {
    Method(FunctionId),
    Imported(ImportedDependencyCallableUseId),
    /// An abstract class intentionally leaves this obligation to a concrete
    /// subclass. The declaration supplies the complete slot signature.
    Abstract {
        declaration: FunctionId,
    },
    ImportedAbstract {
        declaration: ImportedDependencyCallableUseId,
    },
}

#[derive(Debug, Clone)]
pub struct MethodSig {
    pub name: String,
    pub is_suspend: bool,
    pub attributes: FunctionAttributes,
    pub implementation: InterfaceMemberImplementation,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceMemberImplementation {
    Body,
    AbstractSlot,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub identity: PersistentEnumVariantId,
    pub name: String,
    pub gc_free: bool,
    pub fields: Vec<VariantField>,
}

/// A payload field retains its declaration identity across specialization.
#[derive(Debug, Clone)]
pub struct VariantField {
    pub identity: PersistentEnumVariantFieldId,
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct Field {
    /// Declaration identity retained through concretization, including object storage.
    pub identity: PersistentFieldId,
    pub name: String,
    pub ty: TypeId,
}

/// One source-declared struct field with its persistent semantic identity.
/// Keeping the identity in the field makes layout projection structurally
/// total instead of relying on a parallel vector or a field-name lookup.
#[derive(Debug, Clone)]
pub struct DeclaredStructField {
    pub identity: PersistentFieldId,
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    /// Persistent property owner and physical storage role retained across
    /// concretization. MIR/LIR must not reconstruct either from `name`.
    pub storage_owner: PropertyStorageOwner,
    pub ty: TypeId,
    pub mutable: bool,
    pub storage: GlobalStorage,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyStorageOwner {
    Backing(scoop_identity::PropertyOwner),
    Delegate(scoop_identity::PropertyOwner),
    GenericDelegate(GenericDelegateStorageSpecializationId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenericDelegateStorageSpecialization {
    pub storage: GlobalId,
    pub initialization: InitializationUnitId,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Managed {
        state: HirStaticInitialState,
    },
    Local {
        thread_local: bool,
        initializer: HirConstantImage,
    },
    Extern {
        source_contract: Box<SourceNativeExternalContractRecord>,
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirStaticInitialState {
    ZeroedForRuntimeUnit { unit: InitializationUnitId },
    EncodedStaticValue { payload: HirConstantImage },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirConstantImage {
    Integer(HirIntegerConstant),
    Boolean(bool),
    Char(char),
    Float(scoop_identity::FloatConstant),
    String(String),
    NullPointer(HirPointerNullKind),
    EnumUnit {
        variant: EnumVariantRef,
    },
    Struct {
        struct_id: StructId,
        fields: Vec<HirConstantImage>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirPointerNullKind {
    Raw,
    Code,
}

#[cfg(test)]
mod tests;
