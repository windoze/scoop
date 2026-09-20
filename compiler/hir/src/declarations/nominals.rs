use super::*;

/// Lexical declaration owner of a static nested nominal. Each target keeps
/// its own nominal id; the owner relation is typed and never reconstructed
/// from a qualified source name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NominalOwner {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
    Object(ObjectId),
}

#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    /// Application to this declaration's own type parameters (or the empty
    /// application for a parameter-free declaration).
    pub self_application: StructApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    /// Owner parameters that must be recursively GC-free because this
    /// template contains a `Ptr` pointee dependency.
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub attributes: StructAttributes,
    pub representation: StructRepresentation,
    pub constructors: Vec<StructConstructorId>,
    pub interfaces: Vec<TypeId>,
    /// Source-complete mapping from each implemented interface member to the
    /// concrete declaration that implements it. Generic owner/interface
    /// arguments remain in template form and are substituted together.
    pub interface_implementations: Vec<InterfaceImplementation>,
    /// Member declarations in source order. Consumers follow this typed
    /// relation and never recover ownership by scanning `Module::functions`.
    pub methods: Vec<FunctionId>,
    pub properties: Vec<PropertyId>,
    /// Compiler-derived same-type equality declaration, when no explicit
    /// same-signature operator suppresses derivation. Applicability remains
    /// conditional on this application's field obligations.
    pub derived_equality: Option<FunctionId>,
    pub span: Span,
}

impl StructDecl {
    pub fn semantic_fields(&self) -> &[Field] {
        self.representation.semantic_fields()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructApplication {
    pub template: StructId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
    pub representation: StructApplicationRepresentation,
}

#[derive(Debug, Clone)]
pub enum StructRepresentation {
    Declared(Vec<Field>),
    Intrinsic(IntrinsicTypeDeclaration),
}

impl StructRepresentation {
    /// Source-visible fields. Intrinsic types deliberately expose no source
    /// fields even though their compiler representation is not empty.
    pub fn semantic_fields(&self) -> &[Field] {
        match self {
            Self::Declared(fields) => fields,
            Self::Intrinsic(_) => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructApplicationRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeRepresentation),
}

/// Typed struct attributes. Raw annotation names and argument syntax never
/// cross the AST/HIR boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StructAttributes {
    pub no_gc: bool,
    pub c_layout: Option<HirCLayoutContract>,
    pub interior_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirCLayoutValue {
    Natural,
    A1,
    A2,
    A4,
    A8,
    A16,
}

impl HirCLayoutValue {
    pub const fn from_bytes(value: u64) -> Option<Self> {
        match value {
            0 => Some(Self::Natural),
            1 => Some(Self::A1),
            2 => Some(Self::A2),
            4 => Some(Self::A4),
            8 => Some(Self::A8),
            16 => Some(Self::A16),
            _ => None,
        }
    }

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

    pub const fn from_integer(value: HirIntegerConstant) -> Option<Self> {
        match value {
            HirIntegerConstant::Signed64(raw) => Self::from_bytes(raw),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HirCLayoutContract {
    pub aligned: HirCLayoutValue,
    pub packed: HirCLayoutValue,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub self_application: EnumApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub no_gc: bool,
    pub variants: Vec<Variant>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub properties: Vec<PropertyId>,
    pub derived_equality: Option<FunctionId>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumApplication {
    pub template: EnumId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
}

/// Export-side identity of one declaration-local enum variant. The local
/// index is private and can only enter this structure after it has been
/// checked against the owning enum declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantRef {
    enumeration: EnumId,
    local_index: u32,
}

impl EnumVariantRef {
    pub fn checked(enums: &Arena<EnumDecl>, enumeration: EnumId, local_index: u32) -> Option<Self> {
        if enumeration.into_raw().into_u32() as usize >= enums.len() {
            return None;
        }
        enums[enumeration]
            .variants
            .get(local_index as usize)
            .map(|_| Self {
                enumeration,
                local_index,
            })
    }

    pub const fn enumeration(self) -> EnumId {
        self.enumeration
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
}

/// Declaration-local identity of one field of one checked enum variant.
///
/// This ref deliberately retains its variant owner so compiler-recognized
/// contracts cannot pair a bare field ordinal with another variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnumVariantFieldRef {
    variant: EnumVariantRef,
    local_index: u32,
}

impl EnumVariantFieldRef {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        variant: EnumVariantRef,
        local_index: u32,
    ) -> Option<Self> {
        let checked_variant =
            EnumVariantRef::checked(enums, variant.enumeration(), variant.local_index())?;
        if checked_variant != variant {
            return None;
        }
        let enumeration = &enums[checked_variant.enumeration()];
        enumeration
            .variants
            .get(checked_variant.local_index() as usize)?
            .fields
            .get(local_index as usize)
            .map(|_| Self {
                variant: checked_variant,
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

/// Exact export-side identity of one variant of one enum application.
/// Keeping the application and its checked declaration-local variant in one
/// value prevents generic applications from being paired with another
/// template's variant index.
///
/// This is a coordinate identity, not an arena-branded capability. `checked`
/// revalidates both coordinates against the supplied target stores, so a ref
/// from another store is accepted only when those same coordinates form a
/// valid relation in the target stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppliedEnumVariantRef {
    application: EnumApplicationId,
    declaration: EnumVariantRef,
}

impl AppliedEnumVariantRef {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        application: EnumApplicationId,
        declaration: EnumVariantRef,
    ) -> Option<Self> {
        if application.into_raw().into_u32() as usize >= applications.len() {
            return None;
        }
        let checked_declaration = EnumVariantRef::checked(
            enums,
            applications[application].template,
            declaration.local_index(),
        )?;
        (checked_declaration == declaration).then_some(Self {
            application,
            declaration: checked_declaration,
        })
    }

    pub fn checked_index(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        application: EnumApplicationId,
        local_index: u32,
    ) -> Option<Self> {
        if application.into_raw().into_u32() as usize >= applications.len() {
            return None;
        }
        let declaration =
            EnumVariantRef::checked(enums, applications[application].template, local_index)?;
        Self::checked(enums, applications, application, declaration)
    }

    pub const fn application(self) -> EnumApplicationId {
        self.application
    }

    pub const fn declaration(self) -> EnumVariantRef {
        self.declaration
    }

    pub const fn local_index(self) -> u32 {
        self.declaration.local_index()
    }
}

/// Exact export-side identity of one field of one applied enum variant.
/// Like `AppliedEnumVariantRef`, this value has no arena brand: construction
/// rechecks its complete coordinate chain in the supplied target stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppliedEnumVariantFieldRef {
    variant: AppliedEnumVariantRef,
    local_index: u32,
}

impl AppliedEnumVariantFieldRef {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        variant: AppliedEnumVariantRef,
        local_index: u32,
    ) -> Option<Self> {
        let checked_variant = AppliedEnumVariantRef::checked(
            enums,
            applications,
            variant.application(),
            variant.declaration(),
        )?;
        if checked_variant != variant {
            return None;
        }
        let checked_field =
            EnumVariantFieldRef::checked(enums, checked_variant.declaration(), local_index)?;
        Some(Self {
            variant: checked_variant,
            local_index: checked_field.local_index(),
        })
    }

    pub const fn variant(self) -> AppliedEnumVariantRef {
        self.variant
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
    }
}

/// Declaration-side identity of one source-visible field of one struct.
/// Intrinsic struct representations cannot construct this ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructFieldRef {
    structure: StructId,
    local_index: u32,
}

impl StructFieldRef {
    pub fn checked(
        structs: &Arena<StructDecl>,
        structure: StructId,
        local_index: u32,
    ) -> Option<Self> {
        if structure.into_raw().into_u32() as usize >= structs.len() {
            return None;
        }
        structs[structure]
            .semantic_fields()
            .get(local_index as usize)
            .map(|_| Self {
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

/// Exact export-side identity of one source-visible field of one struct
/// application. It retains the checked declaration relation instead of an
/// untyped ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppliedStructFieldRef {
    application: StructApplicationId,
    declaration: StructFieldRef,
}

impl AppliedStructFieldRef {
    pub fn checked(
        structs: &Arena<StructDecl>,
        applications: &Arena<StructApplication>,
        application: StructApplicationId,
        local_index: u32,
    ) -> Option<Self> {
        if application.into_raw().into_u32() as usize >= applications.len() {
            return None;
        }
        let structure = applications[application].template;
        let declaration = StructFieldRef::checked(structs, structure, local_index)?;
        Some(Self {
            application,
            declaration,
        })
    }

    pub const fn application(self) -> StructApplicationId {
        self.application
    }

    pub const fn declaration(self) -> StructFieldRef {
        self.declaration
    }

    pub const fn local_index(self) -> u32 {
        self.declaration.local_index()
    }
}

/// Complete typed identity of the compiler-validated core `Option` contract.
/// `Some` and `None` cannot be paired with variants from another enum or with
/// the wrong payload shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionCore {
    enumeration: EnumId,
    some_payload: EnumVariantFieldRef,
    none: EnumVariantRef,
}

impl OptionCore {
    pub fn checked(
        enums: &Arena<EnumDecl>,
        types: &Arena<Type>,
        some_payload: EnumVariantFieldRef,
        none: EnumVariantRef,
    ) -> Option<Self> {
        let some = some_payload.variant();
        let checked_some = EnumVariantRef::checked(enums, some.enumeration(), some.local_index())?;
        if checked_some != some {
            return None;
        }
        let checked_some_payload =
            EnumVariantFieldRef::checked(enums, checked_some, some_payload.local_index())?;
        if checked_some_payload != some_payload {
            return None;
        }
        let checked_none = EnumVariantRef::checked(enums, none.enumeration(), none.local_index())?;
        if checked_none != none
            || checked_some.enumeration() != checked_none.enumeration()
            || checked_some.local_index() == checked_none.local_index()
        {
            return None;
        }
        let declaration = &enums[checked_some.enumeration()];
        let [parameter] = declaration.type_params.as_slice() else {
            return None;
        };
        if declaration.name != "Option" || declaration.variants.len() != 2 {
            return None;
        }
        let some_variant = declaration
            .variants
            .get(checked_some.local_index() as usize)?;
        let none_variant = declaration
            .variants
            .get(checked_none.local_index() as usize)?;
        let [field] = some_variant.fields.as_slice() else {
            return None;
        };
        if field.ty.into_raw().into_u32() as usize >= types.len() {
            return None;
        }
        if some_variant.name != "Some"
            || none_variant.name != "None"
            || checked_some_payload.local_index() != 0
            || !matches!(types[field.ty], Type::Param(found) if found == parameter.id)
            || !none_variant.fields.is_empty()
        {
            return None;
        }
        Some(Self {
            enumeration: checked_some.enumeration(),
            some_payload: checked_some_payload,
            none: checked_none,
        })
    }

    pub const fn enumeration(self) -> EnumId {
        self.enumeration
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// Effective dispatch modality of a member function (spec 9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodModifier {
    Final,
    Open,
    Abstract,
}

/// Member-only function metadata. Keeping owner and modality together
/// makes it impossible for a method to reach downstream stages without
/// a dispatch modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Method {
    pub owner: TypeId,
    pub modifier: MethodModifier,
    /// Complete source-level dispatch identity. Overrides share a typed
    /// virtual family; interface declarations name their exact member.
    pub dispatch: MethodDispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodDispatch {
    Direct,
    Virtual(VirtualMethodId),
    /// A final override is called directly through its own static type but
    /// still replaces the inherited virtual-family slot for base-typed calls.
    FinalOverride(VirtualMethodId),
    Interface(InterfaceMethodId),
}

/// Closed language-level identity assigned to a successfully validated
/// `operator` declaration. Consumers match this role directly and never
/// recover it from a source name, nominal owner or intrinsic symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    UnaryPlus,
    UnaryMinus,
    Not,
    Inc,
    Dec,
    Plus,
    Minus,
    Times,
    Div,
    Rem,
    RangeTo,
    RangeUntil,
    Contains,
    Get,
    Set,
    Invoke,
    PlusAssign,
    MinusAssign,
    TimesAssign,
    DivAssign,
    RemAssign,
    CompareTo,
    Equals,
    Component { index: std::num::NonZeroU32 },
    Iterator,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub modifier: ClassModifier,
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub self_application: ClassApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub representation: ClassRepresentation,
    pub fields: Vec<ClassFieldId>,
    pub properties: Vec<PropertyId>,
    pub constructors: Vec<ClassConstructorId>,
    pub base_class: Option<TypeId>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
    pub methods: Vec<FunctionId>,
    pub span: Span,
}

/// Typed evaluation plan for one constructor edge. Source argument syntax and
/// missing/default state have already been eliminated.
#[derive(Debug, Clone)]
pub struct ConstructorArguments {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub args: Vec<Expr>,
}

impl ClassDecl {
    pub fn is_declared(&self) -> bool {
        matches!(self.representation, ClassRepresentation::Declared)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassApplication {
    pub template: ClassId,
    pub arguments: Vec<TypeId>,
    pub canonical_type: TypeId,
    pub representation: ClassApplicationRepresentation,
}

#[derive(Debug, Clone)]
pub enum ClassRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeDeclaration),
}

#[derive(Debug, Clone)]
pub struct ClassField {
    pub owner: ClassId,
    pub property: PropertyId,
    pub ty: TypeId,
    pub source: ClassFieldSource,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassFieldSource {
    PrimaryParameter(ConstructorParamId),
    Body,
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
pub struct ClassConstructor {
    pub owner: ClassId,
    /// Whether this callable is a source constructor or the unique generated
    /// zero-argument adapter for another source constructor.
    pub identity_kind: ClassConstructorIdentityKind,
    pub access: DeclarationAccess,
    pub safety: Safety,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: ClassConstructorKind,
    pub span: Span,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassConstructorIdentityKind {
    Source,
    ZeroArgumentAdapter { source: ClassConstructorId },
}

#[derive(Debug, Clone)]
pub enum ClassConstructorKind {
    Primary {
        base: BaseInitialization,
        primary_stores: Vec<PrimaryFieldStore>,
        common_initialization: Vec<ClassInitializationStep>,
    },
    Secondary {
        delegation: ClassSecondaryDelegation,
        body: Body,
    },
}

#[derive(Debug, Clone)]
pub struct PrimaryFieldStore {
    pub field: ClassFieldId,
    pub parameter: ConstructorParamId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassInitializationStep {
    StoredProperty {
        field: ClassFieldId,
        initializer: ConstructorExpression,
        span: Span,
    },
    DelegatedProperty {
        storage: DelegateStorageId,
        field: ClassFieldId,
        initializer: ConstructorExpression,
        span: Span,
    },
    InitBlock {
        body: Body,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ConstructorExpression {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum BaseInitialization {
    Root,
    Super {
        target: ClassConstructorApplicationId,
        arguments: ConstructorArguments,
    },
}

#[derive(Debug, Clone)]
pub enum ClassSecondaryDelegation {
    This {
        target: ClassConstructorApplicationId,
        arguments: ConstructorArguments,
    },
    Terminal {
        base: BaseInitialization,
        common_initialization: Vec<ClassInitializationStep>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassConstructorApplication {
    pub constructor: ClassConstructorId,
    pub owner: ClassApplicationId,
}

#[derive(Debug, Clone)]
pub struct StructConstructor {
    pub owner: StructId,
    pub access: DeclarationAccess,
    pub safety: Safety,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: StructConstructorKind,
    pub span: Span,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone)]
pub enum StructConstructorKind {
    Primary,
    Secondary {
        delegation: StructConstructorDelegation,
        body: Body,
    },
}

#[derive(Debug, Clone)]
pub struct StructConstructorDelegation {
    pub target: StructConstructorApplicationId,
    pub arguments: ConstructorArguments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructConstructorApplication {
    pub constructor: StructConstructorId,
    pub owner: StructApplicationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassApplicationRepresentation {
    Declared,
    Intrinsic(IntrinsicTypeRepresentation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntrinsicTypeDeclaration {
    pub kind: IntrinsicTypeKind,
    pub provider: IntrinsicProviderId,
}

/// Closed semantic identity of every compiler-represented nominal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntrinsicTypeKind {
    Integer(IntegerKind),
    Boolean,
    String,
    Array,
    MutableArray,
    Ptr,
    FunPtr,
}

impl IntrinsicTypeKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Integer(kind) => kind.intrinsic_name(),
            Self::Boolean => "core_boolean",
            Self::String => "core_string",
            Self::Array => "core_array",
            Self::MutableArray => "core_mutable_array",
            Self::Ptr => "core_ptr",
            Self::FunPtr => "core_fun_ptr",
        }
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Integer(kind) => kind.canonical_name(),
            Self::Boolean => "Boolean",
            Self::String => "String",
            Self::Array => "Array",
            Self::MutableArray => "MutableArray",
            Self::Ptr => "Ptr",
            Self::FunPtr => "FunPtr",
        }
    }

    pub const fn target(self) -> IntrinsicTypeTarget {
        match self {
            Self::Integer(_) | Self::Boolean | Self::Ptr | Self::FunPtr => {
                IntrinsicTypeTarget::Struct
            }
            Self::String | Self::Array | Self::MutableArray => IntrinsicTypeTarget::Class,
        }
    }

    pub const fn parameters(self) -> IntrinsicTypeParameters {
        match self {
            Self::Integer(_) | Self::Boolean | Self::String => IntrinsicTypeParameters::None,
            Self::Array | Self::MutableArray => IntrinsicTypeParameters::OneInvariantUnconstrained,
            Self::Ptr => IntrinsicTypeParameters::OneInvariantValue,
            Self::FunPtr => IntrinsicTypeParameters::OneInvariantUnconstrained,
        }
    }

    pub fn application(self, arguments: &[TypeId]) -> IntrinsicTypeRepresentation {
        match (self, arguments) {
            (Self::Integer(kind), []) => IntrinsicTypeRepresentation::Integer(kind),
            (Self::Boolean, []) => IntrinsicTypeRepresentation::Boolean,
            (Self::String, []) => IntrinsicTypeRepresentation::String,
            (Self::Array, [element]) => IntrinsicTypeRepresentation::Array { element: *element },
            (Self::MutableArray, [element]) => {
                IntrinsicTypeRepresentation::MutableArray { element: *element }
            }
            (Self::Ptr, [pointee]) => IntrinsicTypeRepresentation::Ptr { pointee: *pointee },
            (Self::FunPtr, [function]) => IntrinsicTypeRepresentation::FunPtr {
                function: *function,
            },
            _ => unreachable!("HIR validates the intrinsic declaration contract before use"),
        }
    }
}

/// Complete compiler representation of one nominal application. Generic
/// family variants contain their concrete element type directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    Integer(IntegerKind),
    Boolean,
    String,
    Array { element: TypeId },
    MutableArray { element: TypeId },
    Ptr { pointee: TypeId },
    FunPtr { function: TypeId },
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    /// Validated source declaration shape. This is semantic call/pattern
    /// information and the authoritative source of persistent field selectors.
    pub style: VariantStyle,
    /// Fields in declaration order; unit variants have none. Named and
    /// constructor-style fields carry their names, positional fields have
    /// generated `_1`-style names.
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantStyle {
    Unit,
    Positional,
    Named,
    Constructor,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeId,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enum_declaration(
        name: &str,
        self_application: EnumApplicationId,
        variants: Vec<Variant>,
    ) -> EnumDecl {
        EnumDecl {
            name: name.to_string(),
            owner: None,
            access: NominalAccess::public(),
            self_application,
            type_params: Vec::new(),
            gc_free_pointee_requirements: Vec::new(),
            no_gc: false,
            variants,
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: Span::new(0, 0),
        }
    }

    fn struct_declaration(
        name: &str,
        self_application: StructApplicationId,
        representation: StructRepresentation,
    ) -> StructDecl {
        StructDecl {
            name: name.to_string(),
            owner: None,
            access: NominalAccess::public(),
            self_application,
            type_params: Vec::new(),
            gc_free_pointee_requirements: Vec::new(),
            attributes: StructAttributes::default(),
            representation,
            constructors: Vec::new(),
            interfaces: Vec::new(),
            interface_implementations: Vec::new(),
            methods: Vec::new(),
            properties: Vec::new(),
            derived_equality: None,
            span: Span::new(0, 0),
        }
    }

    #[test]
    fn c_layout_accepts_only_canonical_long_values() {
        assert_eq!(
            HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(8)),
            Some(HirCLayoutValue::A8)
        );
        assert_eq!(
            HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(0)),
            Some(HirCLayoutValue::Natural)
        );
        assert_eq!(
            HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(3)),
            None
        );
        assert_eq!(
            HirCLayoutValue::from_integer(HirIntegerConstant::Signed32(8)),
            None
        );
        assert_eq!(
            HirCLayoutValue::from_integer(HirIntegerConstant::Signed64(u64::MAX)),
            None
        );
    }

    #[test]
    fn applied_field_and_variant_refs_close_owner_and_index_relations() {
        let mut types = Arena::new();
        let int = types.alloc(Type::Integer(IntegerKind::SIGNED_32));
        let string = types.alloc(Type::String);
        let parameter = TypeParamId::from_raw(0);
        let parameter_ty = types.alloc(Type::Param(parameter));
        let mut enums = Arena::new();
        let first_application = EnumApplicationId::from_raw(0.into());
        let choice = enums.alloc(enum_declaration(
            "Choice",
            first_application,
            vec![
                Variant {
                    name: "Data".to_string(),
                    style: VariantStyle::Named,
                    fields: vec![Field {
                        name: "value".to_string(),
                        ty: parameter_ty,
                    }],
                },
                Variant {
                    name: "Empty".to_string(),
                    style: VariantStyle::Unit,
                    fields: Vec::new(),
                },
            ],
        ));
        enums[choice].type_params.push(TypeParamDecl {
            id: parameter,
            name: "T".to_string(),
            bounds: TypeParamBounds::Unconstrained,
            span: Span::new(0, 0),
        });
        let other_application = EnumApplicationId::from_raw(3.into());
        let other = enums.alloc(enum_declaration(
            "Other",
            other_application,
            vec![Variant {
                name: "Data".to_string(),
                style: VariantStyle::Unit,
                fields: Vec::new(),
            }],
        ));
        let mut applications = Arena::new();
        let choice_self = applications.alloc(EnumApplication {
            template: choice,
            arguments: vec![parameter_ty],
            canonical_type: parameter_ty,
        });
        assert_eq!(choice_self, first_application);
        let choice_int = applications.alloc(EnumApplication {
            template: choice,
            arguments: vec![int],
            canonical_type: int,
        });
        let choice_string = applications.alloc(EnumApplication {
            template: choice,
            arguments: vec![string],
            canonical_type: string,
        });
        let other_application = applications.alloc(EnumApplication {
            template: other,
            arguments: Vec::new(),
            canonical_type: int,
        });

        let data = EnumVariantRef::checked(&enums, choice, 0).unwrap();
        let empty = EnumVariantRef::checked(&enums, choice, 1).unwrap();
        assert!(EnumVariantRef::checked(&enums, choice, 2).is_none());
        assert!(EnumVariantRef::checked(&enums, EnumId::from_raw(99.into()), 0).is_none());
        let data_field = EnumVariantFieldRef::checked(&enums, data, 0).unwrap();
        assert_eq!(data_field.variant(), data);
        assert_eq!(data_field.local_index(), 0);
        assert!(EnumVariantFieldRef::checked(&enums, data, 1).is_none());
        assert!(EnumVariantFieldRef::checked(&enums, empty, 0).is_none());
        let applied_int =
            AppliedEnumVariantRef::checked(&enums, &applications, choice_int, data).unwrap();
        let applied_string =
            AppliedEnumVariantRef::checked(&enums, &applications, choice_string, data).unwrap();
        assert_ne!(applied_int, applied_string);
        assert!(
            AppliedEnumVariantRef::checked(&enums, &applications, other_application, data)
                .is_none()
        );
        assert!(
            AppliedEnumVariantRef::checked_index(&enums, &applications, choice_int, 2).is_none()
        );
        assert!(
            AppliedEnumVariantRef::checked(
                &enums,
                &applications,
                EnumApplicationId::from_raw(99.into()),
                data
            )
            .is_none()
        );
        assert!(
            AppliedEnumVariantFieldRef::checked(&enums, &applications, applied_int, 0).is_some()
        );
        assert!(
            AppliedEnumVariantFieldRef::checked(&enums, &applications, applied_int, 1).is_none()
        );
        let applied_empty =
            AppliedEnumVariantRef::checked(&enums, &applications, choice_int, empty).unwrap();
        assert!(
            AppliedEnumVariantFieldRef::checked(&enums, &applications, applied_empty, 0).is_none()
        );

        let mut structs = Arena::new();
        let declared_application = StructApplicationId::from_raw(0.into());
        let declared = structs.alloc(struct_declaration(
            "Record",
            declared_application,
            StructRepresentation::Declared(vec![Field {
                name: "value".to_string(),
                ty: parameter_ty,
            }]),
        ));
        structs[declared].type_params.push(TypeParamDecl {
            id: parameter,
            name: "T".to_string(),
            bounds: TypeParamBounds::Unconstrained,
            span: Span::new(0, 0),
        });
        let intrinsic_application = StructApplicationId::from_raw(3.into());
        let intrinsic = structs.alloc(struct_declaration(
            "Intrinsic",
            intrinsic_application,
            StructRepresentation::Intrinsic(IntrinsicTypeDeclaration {
                kind: IntrinsicTypeKind::Boolean,
                provider: IntrinsicProviderId::from_raw(0),
            }),
        ));
        let mut struct_applications = Arena::new();
        let declared_self = struct_applications.alloc(StructApplication {
            template: declared,
            arguments: vec![parameter_ty],
            canonical_type: parameter_ty,
            representation: StructApplicationRepresentation::Declared,
        });
        assert_eq!(declared_self, declared_application);
        let declared_int = struct_applications.alloc(StructApplication {
            template: declared,
            arguments: vec![int],
            canonical_type: int,
            representation: StructApplicationRepresentation::Declared,
        });
        let declared_string = struct_applications.alloc(StructApplication {
            template: declared,
            arguments: vec![string],
            canonical_type: string,
            representation: StructApplicationRepresentation::Declared,
        });
        let intrinsic_application = struct_applications.alloc(StructApplication {
            template: intrinsic,
            arguments: Vec::new(),
            canonical_type: int,
            representation: StructApplicationRepresentation::Intrinsic(
                IntrinsicTypeRepresentation::Boolean,
            ),
        });
        let int_field =
            AppliedStructFieldRef::checked(&structs, &struct_applications, declared_int, 0)
                .expect("Record<Int>.value exists");
        let string_field =
            AppliedStructFieldRef::checked(&structs, &struct_applications, declared_string, 0)
                .expect("Record<String>.value exists");
        assert_ne!(int_field, string_field);
        assert!(
            AppliedStructFieldRef::checked(&structs, &struct_applications, declared_int, 0)
                .is_some()
        );
        assert!(
            AppliedStructFieldRef::checked(&structs, &struct_applications, declared_int, 1)
                .is_none()
        );
        assert!(
            AppliedStructFieldRef::checked(
                &structs,
                &struct_applications,
                intrinsic_application,
                0
            )
            .is_none()
        );
    }

    #[test]
    fn applied_enum_refs_revalidate_coordinates_in_target_stores() {
        let mut types = Arena::new();
        let unit = types.alloc(Type::Unit);

        let mut enums = Arena::new();
        let choice = enums.alloc(enum_declaration(
            "Choice",
            EnumApplicationId::from_raw(0.into()),
            vec![
                Variant {
                    name: "Data".to_string(),
                    style: VariantStyle::Named,
                    fields: vec![Field {
                        name: "value".to_string(),
                        ty: unit,
                    }],
                },
                Variant {
                    name: "Empty".to_string(),
                    style: VariantStyle::Unit,
                    fields: Vec::new(),
                },
            ],
        ));
        let other = enums.alloc(enum_declaration(
            "Other",
            EnumApplicationId::from_raw(1.into()),
            vec![Variant {
                name: "Data".to_string(),
                style: VariantStyle::Named,
                fields: vec![Field {
                    name: "value".to_string(),
                    ty: unit,
                }],
            }],
        ));
        let mut applications = Arena::new();
        let choice_application = applications.alloc(EnumApplication {
            template: choice,
            arguments: Vec::new(),
            canonical_type: unit,
        });
        let other_application = applications.alloc(EnumApplication {
            template: other,
            arguments: Vec::new(),
            canonical_type: unit,
        });

        let mut foreign_enums = Arena::new();
        let foreign_choice = foreign_enums.alloc(enum_declaration(
            "ForeignChoice",
            EnumApplicationId::from_raw(0.into()),
            vec![
                Variant {
                    name: "ForeignData".to_string(),
                    style: VariantStyle::Named,
                    fields: vec![
                        Field {
                            name: "first".to_string(),
                            ty: unit,
                        },
                        Field {
                            name: "second".to_string(),
                            ty: unit,
                        },
                    ],
                },
                Variant {
                    name: "ForeignEmpty".to_string(),
                    style: VariantStyle::Unit,
                    fields: Vec::new(),
                },
                Variant {
                    name: "ForeignExtra".to_string(),
                    style: VariantStyle::Named,
                    fields: vec![Field {
                        name: "value".to_string(),
                        ty: unit,
                    }],
                },
            ],
        ));
        foreign_enums.alloc(enum_declaration(
            "ForeignOther",
            EnumApplicationId::from_raw(2.into()),
            Vec::new(),
        ));
        let mut foreign_applications = Arena::new();
        let foreign_same_application = foreign_applications.alloc(EnumApplication {
            template: foreign_choice,
            arguments: Vec::new(),
            canonical_type: unit,
        });
        let foreign_wrong_owner_application = foreign_applications.alloc(EnumApplication {
            template: foreign_choice,
            arguments: Vec::new(),
            canonical_type: unit,
        });

        assert_eq!(choice_application, foreign_same_application);
        assert_eq!(other_application, foreign_wrong_owner_application);
        let foreign_data = EnumVariantRef::checked(&foreign_enums, foreign_choice, 0).unwrap();
        let foreign_applied = AppliedEnumVariantRef::checked(
            &foreign_enums,
            &foreign_applications,
            foreign_same_application,
            foreign_data,
        )
        .unwrap();

        let target_applied = AppliedEnumVariantRef::checked(
            &enums,
            &applications,
            foreign_applied.application(),
            foreign_applied.declaration(),
        )
        .expect("same coordinates are revalidated in the target stores");
        assert_eq!(target_applied, foreign_applied);
        assert_eq!(
            AppliedEnumVariantFieldRef::checked(&enums, &applications, foreign_applied, 0,)
                .expect("the target variant has field zero")
                .variant(),
            target_applied
        );
        assert!(
            AppliedEnumVariantFieldRef::checked(&enums, &applications, foreign_applied, 1,)
                .is_none(),
            "a field index valid only in the foreign store must be rejected"
        );

        let foreign_wrong_owner = AppliedEnumVariantRef::checked(
            &foreign_enums,
            &foreign_applications,
            foreign_wrong_owner_application,
            foreign_data,
        )
        .unwrap();
        assert!(
            AppliedEnumVariantRef::checked(
                &enums,
                &applications,
                foreign_wrong_owner.application(),
                foreign_wrong_owner.declaration(),
            )
            .is_none(),
            "the target application owner decides the relation"
        );
        assert!(
            AppliedEnumVariantFieldRef::checked(&enums, &applications, foreign_wrong_owner, 0,)
                .is_none(),
            "field construction must revalidate the applied variant owner"
        );

        let foreign_extra = EnumVariantRef::checked(&foreign_enums, foreign_choice, 2).unwrap();
        assert!(
            AppliedEnumVariantRef::checked(
                &enums,
                &applications,
                choice_application,
                foreign_extra,
            )
            .is_none(),
            "a variant index valid only in the foreign store must be rejected"
        );

        let mut invalid_applications = Arena::new();
        let invalid_application = invalid_applications.alloc(EnumApplication {
            template: EnumId::from_raw(99.into()),
            arguments: Vec::new(),
            canonical_type: unit,
        });
        assert!(
            AppliedEnumVariantRef::checked(
                &enums,
                &invalid_applications,
                invalid_application,
                foreign_data,
            )
            .is_none(),
            "an application with an invalid target-store owner must be rejected"
        );
    }

    #[test]
    fn option_core_revalidates_refs_and_field_types_before_indexing() {
        let parameter = TypeParamId::from_raw(7);
        let mut types = Arena::new();
        let parameter_ty = types.alloc(Type::Param(parameter));
        let mut enums = Arena::new();
        let option = enums.alloc(enum_declaration(
            "Option",
            EnumApplicationId::from_raw(0.into()),
            vec![
                Variant {
                    name: "Some".to_string(),
                    style: VariantStyle::Positional,
                    fields: vec![Field {
                        name: "value".to_string(),
                        ty: parameter_ty,
                    }],
                },
                Variant {
                    name: "None".to_string(),
                    style: VariantStyle::Unit,
                    fields: Vec::new(),
                },
            ],
        ));
        enums[option].type_params.push(TypeParamDecl {
            id: parameter,
            name: "T".to_string(),
            bounds: TypeParamBounds::Unconstrained,
            span: Span::new(0, 0),
        });
        let other = enums.alloc(enum_declaration(
            "Other",
            EnumApplicationId::from_raw(1.into()),
            vec![Variant {
                name: "None".to_string(),
                style: VariantStyle::Unit,
                fields: Vec::new(),
            }],
        ));
        let some = EnumVariantRef::checked(&enums, option, 0).unwrap();
        let some_payload = EnumVariantFieldRef::checked(&enums, some, 0).unwrap();
        let none = EnumVariantRef::checked(&enums, option, 1).unwrap();
        assert!(OptionCore::checked(&enums, &types, some_payload, none).is_some());

        let foreign_none = EnumVariantRef::checked(&enums, other, 0).unwrap();
        assert!(OptionCore::checked(&enums, &types, some_payload, foreign_none).is_none());

        let mut foreign_enums = Arena::new();
        let foreign_option = foreign_enums.alloc(enum_declaration(
            "Foreign",
            EnumApplicationId::from_raw(0.into()),
            vec![
                Variant {
                    name: "Payload".to_string(),
                    style: VariantStyle::Named,
                    fields: vec![
                        Field {
                            name: "first".to_string(),
                            ty: parameter_ty,
                        },
                        Field {
                            name: "second".to_string(),
                            ty: parameter_ty,
                        },
                    ],
                },
                Variant {
                    name: "Empty".to_string(),
                    style: VariantStyle::Unit,
                    fields: Vec::new(),
                },
                Variant {
                    name: "Extra".to_string(),
                    style: VariantStyle::Named,
                    fields: vec![Field {
                        name: "value".to_string(),
                        ty: parameter_ty,
                    }],
                },
            ],
        ));
        let foreign_some = EnumVariantRef::checked(&foreign_enums, foreign_option, 0).unwrap();
        let foreign_payload =
            EnumVariantFieldRef::checked(&foreign_enums, foreign_some, 0).unwrap();
        let foreign_none = EnumVariantRef::checked(&foreign_enums, foreign_option, 1).unwrap();
        let from_foreign_coordinates =
            OptionCore::checked(&enums, &types, foreign_payload, foreign_none)
                .expect("foreign refs with valid target coordinates have no arena brand");
        assert_eq!(from_foreign_coordinates.some_payload(), some_payload);
        assert_eq!(from_foreign_coordinates.none(), none);

        let foreign_second_payload =
            EnumVariantFieldRef::checked(&foreign_enums, foreign_some, 1).unwrap();
        assert!(
            OptionCore::checked(&enums, &types, foreign_second_payload, foreign_none).is_none(),
            "a payload index valid only in the foreign store must be rejected"
        );
        let foreign_extra = EnumVariantRef::checked(&foreign_enums, foreign_option, 2).unwrap();
        let foreign_extra_payload =
            EnumVariantFieldRef::checked(&foreign_enums, foreign_extra, 0).unwrap();
        assert!(
            OptionCore::checked(&enums, &types, foreign_extra_payload, foreign_none).is_none(),
            "a variant index valid only in the foreign store must be rejected"
        );

        let empty_types = Arena::new();
        assert!(OptionCore::checked(&enums, &empty_types, some_payload, none).is_none());
        let mut wrong_types = Arena::new();
        wrong_types.alloc(Type::String);
        assert!(OptionCore::checked(&enums, &wrong_types, some_payload, none).is_none());

        let mut invalid_type_enums = enums.clone();
        invalid_type_enums[option].variants[0].fields[0].ty = TypeId::from_raw(99.into());
        assert!(
            OptionCore::checked(&invalid_type_enums, &types, some_payload, none).is_none(),
            "an out-of-bounds field type coordinate must not be indexed"
        );
    }
}
