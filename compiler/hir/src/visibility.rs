use crate::{ClassId, EnumId, FunctionId, InterfaceId, StructId};
use scoop_identity::{ConeIdentity, SourceIdentity};

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

/// Nominal lexical owner used by member-private access. Kinds remain distinct
/// so a coincident arena index cannot grant access to another declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisibilityOwner {
    Class(ClassId),
    Interface(InterfaceId),
    Struct(StructId),
    Enum(EnumId),
    Object(crate::ObjectId),
}

/// One conjunct of an access set. Domains are normalized conjunctions rather
/// than visibility ranks: file, lexical-owner and inheritance regions are
/// incomparable and may be intersected with a Cone restriction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AccessConstraint {
    Cone(ConeIdentity),
    File(SourceIdentity),
    LexicalOwner(VisibilityOwner),
    SubclassesOf(crate::SourceNominalId),
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
        normalized.sort_by(|left, right| match (left, right) {
            (AccessConstraint::SubclassesOf(left), AccessConstraint::SubclassesOf(right)) => {
                left.cmp(right)
            }
            _ => access_constraint_sort_key(left).cmp(&access_constraint_sort_key(right)),
        });
        Self {
            inhabited: true,
            constraints: normalized,
        }
    }

    pub fn intersect(&self, other: &Self) -> Self {
        if !self.inhabited || !other.inhabited {
            return Self::empty();
        }
        Self::from_constraints(self.constraints.iter().chain(&other.constraints).cloned())
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
/// identities with universal lookup domains. Implementations reachable only
/// through inherited public slots remain in the complete graph, not here.
/// M23 serializes this surface and its typed dependency closure.
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
    pub objects: Vec<crate::ObjectId>,
    pub object_types: Vec<crate::ObjectTypeId>,
    pub companion_relations: Vec<crate::CompanionRelationId>,
    pub singleton_values: Vec<crate::SingletonValueId>,
    pub type_aliases: Vec<crate::ExportTypeAliasId>,
}

fn access_constraint_sort_key(
    constraint: &AccessConstraint,
) -> (u8, Option<ConeIdentity>, Option<SourceIdentity>, u8, u32) {
    match constraint {
        AccessConstraint::Cone(cone) => (0, Some(*cone), None, 0, 0),
        AccessConstraint::File(source) => (1, Some(source.cone()), Some(source.clone()), 0, 0),
        AccessConstraint::LexicalOwner(owner) => match owner {
            VisibilityOwner::Class(id) => (2, None, None, 0, id.into_raw().into_u32()),
            VisibilityOwner::Interface(id) => (2, None, None, 1, id.into_raw().into_u32()),
            VisibilityOwner::Struct(id) => (2, None, None, 2, id.into_raw().into_u32()),
            VisibilityOwner::Enum(id) => (2, None, None, 3, id.into_raw().into_u32()),
            VisibilityOwner::Object(id) => (2, None, None, 4, id.into_raw().into_u32()),
        },
        AccessConstraint::SubclassesOf(_) => (3, None, None, 0, 0),
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
pub struct DeclarationAccess {
    pub declared: DeclaredVisibility,
    pub lookup: EffectiveLookupDomain,
    pub slot: Option<SlotContractDomain>,
}

impl DeclarationAccess {
    pub fn public() -> Self {
        Self {
            declared: DeclaredVisibility::Public,
            lookup: EffectiveLookupDomain(AccessDomain::universal()),
            slot: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NominalAccess {
    pub declared: DeclaredVisibility,
    pub lookup: EffectiveLookupDomain,
    pub inheritance: InheritanceDomain,
}

impl NominalAccess {
    pub fn public() -> Self {
        Self {
            declared: DeclaredVisibility::Public,
            lookup: EffectiveLookupDomain(AccessDomain::universal()),
            inheritance: InheritanceDomain(AccessDomain::universal()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_identity::{ConeCoordinate, NormalizedSourcePath};

    fn source(cone: &str, path: &str) -> SourceIdentity {
        let cone = ConeCoordinate::new("test", cone, "0.0.0")
            .unwrap()
            .identity()
            .unwrap();
        SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap()
    }

    #[test]
    fn visibility_distinguishes_semantic_source_identities() {
        let current = |path| source("current", path);
        let first = current("src/first.scoop");
        let second = current("src/second.scoop");
        let core = source("core", "src/first.scoop");
        assert_ne!(first, second);
        assert_ne!(first, core);
        let constraints = [
            AccessConstraint::File(second.clone()),
            AccessConstraint::File(core.clone()),
            AccessConstraint::File(first.clone()),
        ];
        let forward = AccessDomain::from_constraints(constraints.clone());
        let reverse = AccessDomain::from_constraints(constraints.into_iter().rev());
        assert_eq!(forward, reverse);
        assert_eq!(forward.constraints().len(), 3);
    }
}
