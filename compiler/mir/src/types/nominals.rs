use super::*;

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    /// Fixed after all type parameters have been resolved and this MIR
    /// type entity has a complete concrete field list.
    pub gc_free: bool,
    pub representation: StructRepresentation,
}

#[derive(Debug)]
pub enum StructRepresentation {
    Declared(DeclaredStruct),
    Intrinsic(IntrinsicStructRepresentation),
}

#[derive(Debug)]
pub struct DeclaredStruct {
    pub c_layout: Option<MirCLayoutContract>,
    pub c_abi: StructCAbi,
    pub interior_mutable: bool,
    pub fields: Vec<DeclaredStructField>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructCAbi {
    SourceRepresentation,
    UInt64Field {
        field: scoop_identity::PersistentFieldId,
    },
}

impl StructDef {
    /// MIR type whose physical layout is defined by this declaration.
    pub fn physical_type(&self, id: StructId) -> Type {
        match &self.representation {
            StructRepresentation::Declared(DeclaredStruct { .. }) => Type::Struct(id),
            StructRepresentation::Intrinsic(representation) => match representation {
                IntrinsicStructRepresentation::Integer(kind) => Type::Integer(*kind),
                IntrinsicStructRepresentation::Unit => Type::Unit,
                IntrinsicStructRepresentation::Boolean => Type::Boolean,
                IntrinsicStructRepresentation::Char
                | IntrinsicStructRepresentation::Float(_)
                | IntrinsicStructRepresentation::MaybeUninit { .. } => Type::Struct(id),
                IntrinsicStructRepresentation::Ptr { pointee } => {
                    Type::Ptr(Box::new(pointee.clone()))
                }
                IntrinsicStructRepresentation::FunPtr { signature } => Type::FunPtr(*signature),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MirCLayoutValue {
    Natural,
    A1,
    A2,
    A4,
    A8,
    A16,
}

impl MirCLayoutValue {
    pub const fn bytes(self) -> Option<u8> {
        match self {
            Self::Natural => None,
            Self::A1 => Some(1),
            Self::A2 => Some(2),
            Self::A4 => Some(4),
            Self::A8 => Some(8),
            Self::A16 => Some(16),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Natural => "natural",
            Self::A1 => "1",
            Self::A2 => "2",
            Self::A4 => "4",
            Self::A8 => "8",
            Self::A16 => "16",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MirCLayoutContract {
    pub aligned: MirCLayoutValue,
    pub packed: MirCLayoutValue,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// Source-declared struct field retained with its persistent identity for
/// target layout and native ABI projection.
#[derive(Debug)]
pub struct DeclaredStructField {
    pub identity: PersistentFieldId,
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// A class definition with its dispatch layout fixed by mir-lower
/// (impl spec 2.9).
#[derive(Debug)]
pub struct ClassDef {
    pub release_policy: ReleasePolicy,
    pub modifier: ClassModifier,
    pub name: String,
    /// Canonical arguments of this fully specialized application.
    pub type_arguments: Vec<Type>,
    pub representation: ClassRepresentation,
    pub interfaces: Vec<InterfaceId>,
    /// Ordinary virtual methods in vtable order (overrides share the base
    /// slot). Empty vtables are valid and have no implicit prefix.
    pub vtable: Vec<TableSlot>,
    /// itable entries, one per implemented interface (pointer-keyed
    /// lookup at runtime).
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug)]
pub enum ClassRepresentation {
    Declared(DeclaredClass),
    Intrinsic(IntrinsicClassRepresentation),
}

#[derive(Debug)]
pub struct DeclaredClass {
    /// Constructor properties in flattened base-first order.
    pub fields: Vec<Field>,
    pub base_class: Option<ClassId>,
}

impl ClassDef {
    pub fn physical_type(&self, id: ClassId) -> Type {
        match self.representation {
            ClassRepresentation::Intrinsic(IntrinsicClassRepresentation::Any) => Type::Any,
            ClassRepresentation::Intrinsic(IntrinsicClassRepresentation::String) => Type::String,
            _ => Type::Class(id),
        }
    }

    pub fn base_class(&self) -> Option<ClassId> {
        match &self.representation {
            ClassRepresentation::Declared(DeclaredClass { base_class, .. }) => *base_class,
            ClassRepresentation::Intrinsic(_) => None,
        }
    }
}

/// Intrinsic representations whose declarations are value structs.
#[derive(Debug, Clone, PartialEq)]
pub enum IntrinsicStructRepresentation {
    MaybeUninit { value: Type },
    Unit,
    Integer(IntegerKind),
    Boolean,
    Char,
    Float(crate::FloatKind),
    Ptr { pointee: Type },
    FunPtr { signature: FunctionTypeId },
}

/// Intrinsic representations whose declarations are reference classes.
#[derive(Debug, Clone, PartialEq)]
pub enum IntrinsicClassRepresentation {
    Atomic(scoop_identity::AtomicStorage<Type>),
    Any,
    Nothing,
    String,
    Throwable,
    Array { element: Type },
    MutableArray { element: Type },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayKind {
    Immutable,
    Mutable,
}

pub fn array_type<'a>(module: &'a Module, ty: &Type) -> Option<(ArrayKind, &'a Type)> {
    let Type::Class(class) = ty else {
        return None;
    };
    match &module.classes[*class].representation {
        ClassRepresentation::Intrinsic(IntrinsicClassRepresentation::Array { element }) => {
            Some((ArrayKind::Immutable, element))
        }
        ClassRepresentation::Intrinsic(IntrinsicClassRepresentation::MutableArray { element }) => {
            Some((ArrayKind::Mutable, element))
        }
        ClassRepresentation::Declared(DeclaredClass { .. })
        | ClassRepresentation::Intrinsic(
            IntrinsicClassRepresentation::String
            | IntrinsicClassRepresentation::Atomic(_)
            | IntrinsicClassRepresentation::Any
            | IntrinsicClassRepresentation::Nothing
            | IntrinsicClassRepresentation::Throwable,
        ) => None,
    }
}
