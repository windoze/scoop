use super::*;

mod queries;

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub owner: Option<NominalOwner>,
    pub access: NominalAccess,
    pub definition: EnumDefinition,
    pub methods: Vec<FunctionId>,
    pub properties: Vec<PropertyId>,
    pub derived_equality: Option<FunctionId>,
    pub span: Span,
}

/// Checked payload and conformance types, expressed in the declaration's
/// binder domain. Every application substitutes this same definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDefinition {
    pub gc_free_pointee_requirements: Vec<RequiresGcFreePointee>,
    pub no_gc: bool,
    pub self_application: EnumApplicationId,
    pub type_params: Vec<TypeParamDecl>,
    pub variants: Vec<Variant>,
    pub interfaces: Vec<TypeId>,
    pub interface_implementations: Vec<InterfaceImplementation>,
}

impl std::ops::Deref for EnumDecl {
    type Target = EnumDefinition;

    fn deref(&self) -> &Self::Target {
        &self.definition
    }
}

impl std::ops::DerefMut for EnumDecl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.definition
    }
}

/// A decoded definition retains the provider's declaration and uses the same
/// symbolic payload types as a definition produced from current source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedEnumDefinition {
    pub declaration: std::sync::Arc<ImportedNominalDeclaration>,
    pub definition: EnumDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumApplication {
    pub template: SourceNominalId,
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
        nominal_identities: &HirNominalIdentities,
        application: EnumApplicationId,
        declaration: EnumVariantRef,
    ) -> Option<Self> {
        if application.into_raw().into_u32() as usize >= applications.len() {
            return None;
        }
        let enumeration = nominal_identities.enum_id(applications[application].template)?;
        let checked_declaration =
            EnumVariantRef::checked(enums, enumeration, declaration.local_index())?;
        (checked_declaration == declaration).then_some(Self {
            application,
            declaration: checked_declaration,
        })
    }

    pub fn checked_index(
        enums: &Arena<EnumDecl>,
        applications: &Arena<EnumApplication>,
        nominal_identities: &HirNominalIdentities,
        application: EnumApplicationId,
        local_index: u32,
    ) -> Option<Self> {
        if application.into_raw().into_u32() as usize >= applications.len() {
            return None;
        }
        let enumeration = nominal_identities.enum_id(applications[application].template)?;
        let declaration = EnumVariantRef::checked(enums, enumeration, local_index)?;
        Self::checked(
            enums,
            applications,
            nominal_identities,
            application,
            declaration,
        )
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
        nominal_identities: &HirNominalIdentities,
        variant: AppliedEnumVariantRef,
        local_index: u32,
    ) -> Option<Self> {
        let checked_variant = AppliedEnumVariantRef::checked(
            enums,
            applications,
            nominal_identities,
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

    pub const fn declaration(self) -> EnumVariantFieldRef {
        EnumVariantFieldRef {
            variant: self.variant.declaration(),
            local_index: self.local_index,
        }
    }

    pub const fn local_index(self) -> u32 {
        self.local_index
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
