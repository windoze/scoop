use super::*;

#[derive(Debug, Clone)]
pub struct StructDef {
    pub origin: StructOriginId,
    pub name: String,
    pub type_arguments: Vec<TypeId>,
    pub gc_free: bool,
    pub representation: StructRepresentation,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StructRepresentation {
    Declared {
        attributes: StructAttributes,
        fields: Vec<Field>,
    },
    Intrinsic {
        declaration: IntrinsicTypeDeclaration,
        application: IntrinsicTypeRepresentation,
    },
}

impl StructDef {
    pub fn declared_fields(&self) -> &[Field] {
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
    pub origin: EnumOriginId,
    pub name: String,
    pub type_arguments: Vec<TypeId>,
    pub gc_free: bool,
    pub variants: Vec<Variant>,
    pub option_variants: Option<(VariantId, VariantId)>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub origin: ClassOriginId,
    pub modifier: ClassModifier,
    pub name: String,
    pub type_arguments: Vec<TypeId>,
    pub representation: ClassRepresentation,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

/// Complete signature and owning class of one compiler-hidden constructor.
/// This is a callable entity in LocalConcreteHir, not a request for MIR to
/// discover or synthesize a target from the class name or id.
#[derive(Debug, Clone)]
pub struct ClassConstructor {
    pub class: ClassId,
    pub source_discriminator: u32,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: ClassConstructorKind,
}

#[derive(Debug, Clone)]
pub struct ConstructorParameter {
    pub id: ConstructorParamId,
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
    pub source_discriminator: u32,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: StructConstructorKind,
}

#[derive(Debug, Clone)]
pub enum StructConstructorKind {
    Primary,
    Secondary {
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
        declaration: IntrinsicTypeDeclaration,
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
    Int,
    UInt,
    Boolean,
    String,
    Array { element: TypeId },
    MutableArray { element: TypeId },
}

#[derive(Debug, Clone)]
pub struct InterfaceDef {
    pub origin: InterfaceOriginId,
    pub name: String,
    pub family: InterfaceFamilyId,
    /// Variance and arguments are copied onto every concrete application.
    /// MIR therefore never consults the generic interface template.
    pub variances: Vec<Variance>,
    pub type_arguments: Vec<TypeId>,
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
    /// An abstract class intentionally leaves this obligation to a concrete
    /// subclass. The declaration supplies the complete slot signature.
    Abstract {
        declaration: FunctionId,
    },
}

#[derive(Debug, Clone)]
pub struct MethodSig {
    pub name: String,
    pub is_suspend: bool,
    pub attributes: FunctionAttributes,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub gc_free: bool,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
    pub storage: GlobalStorage,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    Local {
        thread_local: bool,
        initializer: ConstantValue,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i64),
    Bool(bool),
    NullPtr,
    NullFunPtr,
    Struct {
        struct_id: StructId,
        fields: Vec<ConstantValue>,
    },
}
