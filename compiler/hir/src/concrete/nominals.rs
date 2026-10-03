use super::*;
use crate::HirNominalIdentity;
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
    Declared {
        attributes: StructAttributes,
        c_abi: StructCAbi,
        fields: Vec<DeclaredStructField>,
    },
    Intrinsic {
        declaration: IntrinsicTypeKind,
        application: IntrinsicTypeRepresentation,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructCAbi {
    SourceRepresentation,
    UInt64Field {
        field: scoop_identity::PersistentFieldId,
    },
}

impl StructDef {
    pub fn declared_fields(&self) -> &[DeclaredStructField] {
        match &self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic { .. } => {
                panic!("an intrinsic struct has no source field representation")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EnumDef {
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

/// Checked local-concrete identity of one enum variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantRef {
    enumeration: EnumId,
    variant: VariantId,
}

impl EnumVariantRef {
    pub fn checked(
        enums: &Arena<EnumDef>,
        enumeration: EnumId,
        variant: VariantId,
    ) -> Option<Self> {
        if enumeration.into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        enums[enumeration]
            .variants
            .get(variant.into_raw() as usize)
            .map(|_| Self {
                enumeration,
                variant,
            })
    }

    pub const fn enumeration(self) -> EnumId {
        self.enumeration
    }

    pub const fn variant(self) -> VariantId {
        self.variant
    }
}

/// Checked local-concrete identity of one payload field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantFieldRef {
    variant: EnumVariantRef,
    local_index: u32,
}

impl EnumVariantFieldRef {
    pub fn checked(
        enums: &Arena<EnumDef>,
        variant: EnumVariantRef,
        local_index: u32,
    ) -> Option<Self> {
        if variant.enumeration().into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        let declaration = enums[variant.enumeration()]
            .variants
            .get(variant.variant().into_raw() as usize)?;
        declaration.fields.get(local_index as usize).map(|_| Self {
            variant,
            local_index,
        })
    }

    pub const fn variant(self) -> EnumVariantRef {
        self.variant
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
}

/// Complete local-concrete identity of one specialized core `Option` shape.
/// The Some payload and None variants are inseparable from their checked
/// owner, and their arities are verified when this value is constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptionCore {
    some_payload: EnumVariantFieldRef,
    none: EnumVariantRef,
}

impl OptionCore {
    pub fn checked(
        enums: &Arena<EnumDef>,
        some_payload: EnumVariantFieldRef,
        none: EnumVariantRef,
    ) -> Option<Self> {
        let some = some_payload.variant();
        if some.enumeration() != none.enumeration() || some.variant() == none.variant() {
            return None;
        }
        if some.enumeration().into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        let enumeration = &enums[some.enumeration()];
        let [argument] = enumeration.type_arguments.as_slice() else {
            return None;
        };
        let some_definition = enumeration
            .variants
            .get(some.variant().into_raw() as usize)?;
        let none_definition = enumeration
            .variants
            .get(none.variant().into_raw() as usize)?;
        (enumeration.variants.len() == 2
            && some_definition.name == "Some"
            && some_definition.fields.len() == 1
            && some_payload.local_index() == 0
            && some_definition
                .fields
                .get(some_payload.local_index() as usize)
                .is_some_and(|field| field.ty == *argument)
            && none_definition.name == "None"
            && none_definition.fields.is_empty())
        .then_some(Self { some_payload, none })
    }

    pub const fn enumeration(self) -> EnumId {
        self.some_payload.variant().enumeration()
    }

    pub const fn some(self) -> EnumVariantRef {
        self.some_payload.variant()
    }

    pub const fn some_payload(self) -> EnumVariantFieldRef {
        self.some_payload
    }

    pub const fn none(self) -> EnumVariantRef {
        self.none
    }
}

/// Checked local-concrete identity of one declared struct field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructFieldRef {
    structure: StructId,
    local_index: u32,
}

impl StructFieldRef {
    pub fn checked(
        structs: &Arena<StructDef>,
        structure: StructId,
        local_index: u32,
    ) -> Option<Self> {
        if structure.into_raw().into_u32() as usize >= structs.len() {
            return None;
        }
        let StructRepresentation::Declared { fields, .. } = &structs[structure].representation
        else {
            return None;
        };
        fields.get(local_index as usize).map(|_| Self {
            structure,
            local_index,
        })
    }

    pub const fn structure(self) -> StructId {
        self.structure
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
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
    Declared {
        fields: Vec<Field>,
        base_class: Option<ClassId>,
    },
    Intrinsic {
        declaration: IntrinsicTypeKind,
        application: IntrinsicTypeRepresentation,
    },
}

impl ClassDef {
    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            ClassRepresentation::Declared { fields, .. } => fields,
            ClassRepresentation::Intrinsic { .. } => {
                panic!("an intrinsic class has no source constructor representation")
            }
        }
    }

    pub fn base_class(&self) -> Option<ClassId> {
        match &self.representation {
            ClassRepresentation::Declared { base_class, .. } => *base_class,
            ClassRepresentation::Intrinsic { .. } => None,
        }
    }
}

/// Complete concrete representation selected by an intrinsic declaration and
/// this application's already-lowered arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    Integer(IntegerKind),
    Boolean,
    String,
    Array { element: TypeId },
    MutableArray { element: TypeId },
    Ptr { pointee: TypeId },
    FunPtr { signature: FunctionTypeId },
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
