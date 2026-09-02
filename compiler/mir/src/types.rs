use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Int,
    /// Unsigned 64-bit integer (`UInt`, spec 11.2; same machine word
    /// as `Int`, mapped to `i64` at LIR).
    UInt,
    Boolean,
    String,
    Struct(StructId),
    /// A reference type declared with `class`.
    Class(ClassId),
    /// An interface type (dispatch through itables, impl spec 2.9).
    Interface(InterfaceId),
    /// The root of all types; boxed value types live behind it.
    Any,
    Tuple(Vec<Type>),
    /// Concrete managed function signature. Function values have reference
    /// representation; closure classes are materialized by M11 conversion.
    Function(FunctionTypeId),
    /// Typed raw data pointer; representation is one native pointer word.
    Ptr(Box<Type>),
    /// Typed C function pointer; identity includes its exact signature.
    FunPtr(FunctionTypeId),
    /// An instantiated enum type (including `Option<T>` since M4).
    Enum(EnumId, Vec<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    pub is_suspend: bool,
    pub parameter_types: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug)]
pub struct ClosureClass {
    pub name: String,
    pub function_type: FunctionTypeId,
    pub invoke: ClosureInvokeFunctionId,
    pub captures: Vec<Field>,
    /// Function-type views supported by this exact closure class. Each slot
    /// is a typed forwarding entry whose ABI is `target`.
    pub bridges: Vec<FunctionBridge>,
}

#[derive(Debug)]
pub struct FunctionBridge {
    pub target: FunctionTypeId,
    pub function: FunctionId,
}

#[derive(Debug)]
pub struct ClosureInvokeFunction {
    pub function: FunctionId,
}

/// Typed identity reserved for variance bridges. M11's variance gate fills
/// this arena; keeping it distinct now prevents adapters from being confused
/// with source closure classes.
#[derive(Debug)]
pub struct ClosureAdapter {
    pub class: ClosureClassId,
    pub source: FunctionTypeId,
    pub target: FunctionTypeId,
}

/// Adapter used after a runtime `Any`/interface-to-function check. Its source
/// signature is discovered from the captured closure's TypeDescriptor bridge
/// table, while its exposed invoke ABI is exactly `target`.
#[derive(Debug)]
pub struct DynamicClosureAdapter {
    pub class: ClosureClassId,
    pub target: FunctionTypeId,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    /// Fixed after all type parameters have been resolved and this MIR
    /// type entity has a complete concrete field list.
    pub gc_free: bool,
    pub representation: StructRepresentation,
}

#[derive(Debug)]
pub enum StructRepresentation {
    Declared {
        c_layout: Option<CLayout>,
        interior_mutable: bool,
        fields: Vec<Field>,
    },
    Intrinsic(IntrinsicTypeRepresentation),
}

impl StructDef {
    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }

    pub fn declared_fields_mut(&mut self) -> &mut Vec<Field> {
        match &mut self.representation {
            StructRepresentation::Declared { fields, .. } => fields,
            StructRepresentation::Intrinsic(_) => {
                panic!("an intrinsic struct has no declared field representation")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CLayout {
    pub aligned: u8,
    pub packed: u8,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// An instantiated enum definition (M4): variants with concrete field
/// types. `name` is the mangled instance name (e.g. `Option$I`).
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    /// True exactly when every fully specialized variant is GC-free.
    pub gc_free: bool,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug)]
pub struct VariantDef {
    pub name: String,
    /// GC-free classification of this fully specialized variant.
    pub gc_free: bool,
    /// Fields in declaration order (named and positional forms both
    /// normalized; positional fields carry `_1`-style names).
    pub fields: Vec<Field>,
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
    pub modifier: ClassModifier,
    pub name: String,
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
    Declared {
        /// Constructor properties in flattened base-first order.
        fields: Vec<Field>,
        base_class: Option<ClassId>,
    },
    Intrinsic(IntrinsicTypeRepresentation),
}

impl ClassDef {
    pub fn declared_fields(&self) -> &[Field] {
        match &self.representation {
            ClassRepresentation::Declared { fields, .. } => fields,
            ClassRepresentation::Intrinsic(_) => {
                panic!("an intrinsic class has no declared field representation")
            }
        }
    }

    pub fn declared_fields_mut(&mut self) -> &mut Vec<Field> {
        match &mut self.representation {
            ClassRepresentation::Declared { fields, .. } => fields,
            ClassRepresentation::Intrinsic(_) => {
                panic!("an intrinsic class has no declared field representation")
            }
        }
    }

    pub fn base_class(&self) -> Option<ClassId> {
        match &self.representation {
            ClassRepresentation::Declared { base_class, .. } => *base_class,
            ClassRepresentation::Intrinsic(_) => None,
        }
    }
}

/// The exact compiler representation selected upstream for this fully
/// specialized nominal type. Family variants carry their MIR element type.
#[derive(Debug, Clone, PartialEq)]
pub enum IntrinsicTypeRepresentation {
    Int,
    UInt,
    Boolean,
    String,
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
        ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
            Some((ArrayKind::Immutable, element))
        }
        ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray { element }) => {
            Some((ArrayKind::Mutable, element))
        }
        ClassRepresentation::Declared { .. }
        | ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String) => None,
        ClassRepresentation::Intrinsic(_) => {
            unreachable!("the intrinsic registry fixes declaration targets")
        }
    }
}

#[derive(Debug)]
pub enum TableSlot {
    Function(FunctionId),
    Runtime(RuntimeFn),
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface: InterfaceId,
    pub slots: Vec<TableSlot>,
}

#[derive(Debug)]
pub struct InterfaceDef {
    pub name: String,
    /// Signature-only method declarations in itable-slot order.
    pub methods: Vec<FunctionId>,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}
