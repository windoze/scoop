use crate::{ClassId, EnumId, FunctionId, InterfaceId, IntrinsicProviderId, StructId, TypeId};

/// Source visibility after AST omission has been normalized. There is no
/// `Omitted` state in HIR: every declaration has made the language default
/// (`internal`) explicit before it enters the semantic graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclaredVisibility {
    Public,
    Internal,
    Private,
    Protected,
}

/// Stable identity of one source file inside a provider/Cone. A file number
/// alone is deliberately insufficient at an export boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VisibilityFile {
    pub provider: IntrinsicProviderId,
    pub index: u32,
}

/// Nominal lexical owner used by member-private access. Kinds remain distinct
/// so a coincident arena index cannot grant access to another declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisibilityOwner {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
}

/// One conjunct of an access set. Domains are normalized conjunctions rather
/// than visibility ranks: file, lexical-owner and inheritance regions are
/// incomparable and may be intersected with a Cone restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessConstraint {
    Cone(IntrinsicProviderId),
    File(VisibilityFile),
    LexicalOwner(VisibilityOwner),
    SubclassesOf(ClassId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessDomain {
    inhabited: bool,
    constraints: Vec<AccessConstraint>,
}

impl AccessDomain {
    pub fn universal() -> Self {
        Self {
            inhabited: true,
            constraints: Vec::new(),
        }
    }

    pub fn empty() -> Self {
        Self {
            inhabited: false,
            constraints: Vec::new(),
        }
    }

    pub fn from_constraints(constraints: impl IntoIterator<Item = AccessConstraint>) -> Self {
        let mut normalized = Vec::new();
        for constraint in constraints {
            if !normalized.contains(&constraint) {
                normalized.push(constraint);
            }
        }
        normalized.sort_by_key(access_constraint_sort_key);
        Self {
            inhabited: true,
            constraints: normalized,
        }
    }

    pub fn intersect(&self, other: &Self) -> Self {
        if !self.inhabited || !other.inhabited {
            return Self::empty();
        }
        Self::from_constraints(self.constraints.iter().chain(&other.constraints).copied())
    }

    pub fn constraints(&self) -> &[AccessConstraint] {
        &self.constraints
    }

    pub fn is_empty(&self) -> bool {
        !self.inhabited
    }

    pub fn is_universal(&self) -> bool {
        self.inhabited && self.constraints.is_empty()
    }
}

/// Authoritative cross-Cone declaration surface. The enclosing Export HIR
/// retains the complete current-Cone graph for concretization, but consumers
/// may discover source declarations only through these explicitly public
/// identities. M23 serializes this surface and its typed dependency closure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PublicSemanticSurface {
    pub functions: Vec<FunctionId>,
    pub properties: Vec<crate::PropertyId>,
    pub property_getters: Vec<crate::PropertyGetterId>,
    pub property_setters: Vec<crate::PropertySetterId>,
    pub generic_functions: Vec<crate::GenericFunctionId>,
    pub generic_methods: Vec<crate::GenericMethodId>,
    pub structs: Vec<StructId>,
    pub struct_constructors: Vec<crate::StructConstructorId>,
    pub enums: Vec<EnumId>,
    pub classes: Vec<ClassId>,
    pub class_constructors: Vec<crate::ClassConstructorId>,
    pub interfaces: Vec<InterfaceId>,
    pub interface_methods: Vec<crate::InterfaceMethodId>,
}

fn access_constraint_sort_key(constraint: &AccessConstraint) -> (u8, u32, u32) {
    match *constraint {
        AccessConstraint::Cone(provider) => (0, provider.into_raw(), 0),
        AccessConstraint::File(file) => (1, file.provider.into_raw(), file.index),
        AccessConstraint::LexicalOwner(owner) => match owner {
            VisibilityOwner::Class(id) => (2, 0, id.into_raw().into_u32()),
            VisibilityOwner::Interface(id) => (2, 1, id.into_raw().into_u32()),
            VisibilityOwner::Struct(id) => (2, 2, id.into_raw().into_u32()),
            VisibilityOwner::Enum(id) => (2, 3, id.into_raw().into_u32()),
        },
        AccessConstraint::SubclassesOf(id) => (3, id.into_raw().into_u32(), 0),
    }
}

/// Domain used for direct name/member lookup after intersecting every owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveLookupDomain(pub AccessDomain);

/// Places where a nominal declaration may acquire legal subclasses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InheritanceDomain(pub AccessDomain);

/// Callable/property slot coverage. This is intentionally not convertible to
/// `EffectiveLookupDomain`: overrides may carry a public slot through a
/// narrower concrete owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotContractDomain(pub AccessDomain);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyOverrideAccessWitness {
    pub overriding: crate::PropertyId,
    pub inherited: crate::PropertyId,
    pub required: SlotContractDomain,
    pub provided: SlotContractDomain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationAccess {
    pub declared: DeclaredVisibility,
    pub lookup: EffectiveLookupDomain,
    pub slot: Option<SlotContractDomain>,
    pub signature: Vec<SignatureExposureWitness>,
}

impl DeclarationAccess {
    pub fn public() -> Self {
        Self {
            declared: DeclaredVisibility::Public,
            lookup: EffectiveLookupDomain(AccessDomain::universal()),
            slot: None,
            signature: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NominalAccess {
    pub declared: DeclaredVisibility,
    pub lookup: EffectiveLookupDomain,
    pub inheritance: InheritanceDomain,
    pub signature: Vec<SignatureExposureWitness>,
}

impl NominalAccess {
    pub fn public() -> Self {
        Self {
            declared: DeclaredVisibility::Public,
            lookup: EffectiveLookupDomain(AccessDomain::universal()),
            inheritance: InheritanceDomain(AccessDomain::universal()),
            signature: Vec::new(),
        }
    }
}

/// Proof stored on a successful direct lookup. The declaration id stays
/// separate so this witness cannot be reused as an override proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupAccessWitness {
    pub declaration: AccessDeclaration,
    pub domain: EffectiveLookupDomain,
    pub site: VisibilityFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessDeclaration {
    Function(FunctionId),
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
}

/// Proof that an override declaration covers one inherited slot contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideAccessWitness {
    pub overriding: FunctionId,
    pub inherited: FunctionId,
    pub required: SlotContractDomain,
    pub provided: SlotContractDomain,
}

/// Proof that one signature dependency covers both direct lookup and slot
/// consumers. It is distinct from lookup/default witnesses by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureExposureWitness {
    pub dependency: TypeId,
    pub required: AccessDomain,
    pub provided: AccessDomain,
}
