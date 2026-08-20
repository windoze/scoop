use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{PersistentEnumVariantId, PersistentExactTypeId};

use super::{
    CanonicalExactTypeFactsV1, ExactTypeFactsV1, ExactTypeGcV1, ExactTypeKindV1, ZstStatus,
};

mod dependencies;
pub use dependencies::*;

/// Representation-independent input obtained from checked exact keys and
/// source representation records. A managed reference is a leaf: its object
/// fields do not participate in the GC or zero-sized status of the value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactTypeFactShapeV1 {
    Unit,
    Scalar,
    Pointer,
    Reference,
    MaybeUninit {
        value: PersistentExactTypeId,
    },
    OrdinaryStruct {
        fields: Vec<PersistentExactTypeId>,
    },
    CLayoutStruct {
        fields: Vec<PersistentExactTypeId>,
    },
    Tuple {
        elements: Vec<PersistentExactTypeId>,
    },
    Enum {
        variants: Vec<ExactEnumVariantFactsV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEnumVariantFactsV1 {
    pub variant: PersistentEnumVariantId,
    pub fields: Vec<PersistentExactTypeId>,
    pub gc: ExactTypeGcV1,
}

/// Implemented by the validated exact/source representation closure. This
/// authority does not consult target layouts or native-boundary witnesses.
pub trait ExactTypeFactsSemanticAuthority<E> {
    fn fact_shape(&self, exact: PersistentExactTypeId) -> Result<&ExactTypeFactShapeV1, E>;
}

#[derive(Clone, Copy, Debug)]
pub struct CheckedExactTypeFactsV1<'a> {
    facts: &'a CanonicalExactTypeFactsV1,
}

impl CheckedExactTypeFactsV1<'_> {
    pub fn get(&self, exact: PersistentExactTypeId) -> Option<&ExactTypeFactsV1> {
        self.facts.get(exact)
    }
    pub fn records(&self) -> &[ExactTypeFactsV1] {
        self.facts.records()
    }
}

impl CanonicalExactTypeFactsV1 {
    pub fn validate_semantics<'a, A, E>(
        &'a self,
        authority: &A,
    ) -> Result<CheckedExactTypeFactsV1<'a>, ExactTypeFactsSemanticError<E>>
    where
        A: ExactTypeFactsSemanticAuthority<E>,
    {
        self.validate_semantics_with_dependencies(authority, &dependencies::NoDependencies)
    }

    pub fn validate_semantics_with_dependencies<'a, A, E>(
        &'a self,
        authority: &A,
        dependencies: &dyn ExactTypeFactsDependencyLookupV1,
    ) -> Result<CheckedExactTypeFactsV1<'a>, ExactTypeFactsSemanticError<E>>
    where
        A: ExactTypeFactsSemanticAuthority<E>,
    {
        let mut validation = Validation {
            facts: self,
            authority,
            dependencies,

            active: BTreeSet::new(),
            complete: BTreeMap::new(),
        };
        for record in self.records() {
            validation.visit(record.exact())?;
        }
        Ok(CheckedExactTypeFactsV1 { facts: self })
    }
}

struct Validation<'a, A> {
    facts: &'a CanonicalExactTypeFactsV1,
    authority: &'a A,
    dependencies: &'a dyn ExactTypeFactsDependencyLookupV1,

    active: BTreeSet<PersistentExactTypeId>,
    complete: BTreeMap<PersistentExactTypeId, ExactTypeFactsV1>,
}

impl<A> Validation<'_, A> {
    fn visit<E>(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<ExactTypeFactsV1, ExactTypeFactsSemanticError<E>>
    where
        A: ExactTypeFactsSemanticAuthority<E>,
    {
        if let Some(facts) = self.complete.get(&exact) {
            return Ok(*facts);
        }
        let Some(actual) = self.facts.get(exact) else {
            let fact = self
                .dependencies
                .get_dependency_fact(exact)
                .ok_or(ExactTypeFactsSemanticError::MissingFacts(exact))?
                .record();
            if fact.exact() != exact {
                return Err(ExactTypeFactsSemanticError::DependencyIdentity {
                    expected: exact,
                    actual: fact.exact(),
                });
            }
            return Ok(*fact);
        };
        if self.active.contains(&exact) {
            return Err(ExactTypeFactsSemanticError::ByValueCycle(exact));
        }

        self.active.insert(exact);
        let shape = self
            .authority
            .fact_shape(exact)
            .map_err(|error| ExactTypeFactsSemanticError::Shape { exact, error })?;
        let (kind, gc) = match shape {
            ExactTypeFactShapeV1::Unit => (value(ZstStatus::ZeroSized), ExactTypeGcV1::GcFree),
            ExactTypeFactShapeV1::Scalar | ExactTypeFactShapeV1::Pointer => {
                (value(ZstStatus::NonZero), ExactTypeGcV1::GcFree)
            }
            ExactTypeFactShapeV1::Reference => (
                ExactTypeKindV1::Reference,
                ExactTypeGcV1::ContainsManagedReferences,
            ),
            ExactTypeFactShapeV1::OrdinaryStruct { fields }
            | ExactTypeFactShapeV1::Tuple { elements: fields } => self.fields(fields)?,
            ExactTypeFactShapeV1::MaybeUninit { value } => self.fields(&[*value])?,
            ExactTypeFactShapeV1::CLayoutStruct { fields } => {
                if fields.is_empty() {
                    return Err(ExactTypeFactsSemanticError::EmptyCLayout(exact));
                }
                for field in fields {
                    let facts = self.visit(*field)?;
                    if facts.kind() == value(ZstStatus::ZeroSized) {
                        return Err(ExactTypeFactsSemanticError::ZeroSizedCLayoutField {
                            owner: exact,
                            field: *field,
                        });
                    }
                }
                let (_, gc) = self.fields(fields)?;
                (value(ZstStatus::NonZero), gc)
            }
            ExactTypeFactShapeV1::Enum { variants } => {
                let mut gc_free = true;
                let mut ids = BTreeSet::new();

                for variant in variants {
                    if !ids.insert(variant.variant) {
                        return Err(ExactTypeFactsSemanticError::DuplicateVariant {
                            owner: exact,
                            variant: variant.variant,
                        });
                    }
                    let (_, gc) = self.fields(&variant.fields)?;
                    if gc != variant.gc {
                        return Err(ExactTypeFactsSemanticError::VariantGc {
                            owner: exact,
                            variant: variant.variant,
                            expected: gc,
                            actual: variant.gc,
                        });
                    }
                    gc_free &= gc.is_gc_free();
                }
                (value(ZstStatus::NonZero), gc(gc_free))
            }
        };
        if actual.kind() != kind || actual.gc() != gc {
            return Err(ExactTypeFactsSemanticError::Mismatch {
                exact,
                expected_kind: kind,
                expected_gc: gc,
                actual: *actual,
            });
        }
        self.active.remove(&exact);
        self.complete.insert(exact, *actual);
        Ok(*actual)
    }

    fn fields<E>(
        &mut self,
        fields: &[PersistentExactTypeId],
    ) -> Result<(ExactTypeKindV1, ExactTypeGcV1), ExactTypeFactsSemanticError<E>>
    where
        A: ExactTypeFactsSemanticAuthority<E>,
    {
        let mut zero_sized = true;
        let mut gc_free = true;
        for field in fields {
            let facts = self.visit(*field)?;
            zero_sized &= facts.kind() == value(ZstStatus::ZeroSized);
            gc_free &= facts.gc().is_gc_free();
        }
        Ok((
            value(if zero_sized {
                ZstStatus::ZeroSized
            } else {
                ZstStatus::NonZero
            }),
            gc(gc_free),
        ))
    }
}

const fn value(zst: ZstStatus) -> ExactTypeKindV1 {
    ExactTypeKindV1::Value { zst }
}
const fn gc(gc_free: bool) -> ExactTypeGcV1 {
    if gc_free {
        ExactTypeGcV1::GcFree
    } else {
        ExactTypeGcV1::ContainsManagedReferences
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExactTypeFactsSemanticError<E> {
    Shape {
        exact: PersistentExactTypeId,
        error: E,
    },
    MissingFacts(PersistentExactTypeId),
    DependencyIdentity {
        expected: PersistentExactTypeId,
        actual: PersistentExactTypeId,
    },
    ByValueCycle(PersistentExactTypeId),
    EmptyCLayout(PersistentExactTypeId),
    ZeroSizedCLayoutField {
        owner: PersistentExactTypeId,
        field: PersistentExactTypeId,
    },
    DuplicateVariant {
        owner: PersistentExactTypeId,
        variant: PersistentEnumVariantId,
    },
    VariantGc {
        owner: PersistentExactTypeId,
        variant: PersistentEnumVariantId,
        expected: ExactTypeGcV1,
        actual: ExactTypeGcV1,
    },
    Mismatch {
        exact: PersistentExactTypeId,
        expected_kind: ExactTypeKindV1,
        expected_gc: ExactTypeGcV1,
        actual: ExactTypeFactsV1,
    },
}

impl<E: fmt::Display> fmt::Display for ExactTypeFactsSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape { exact, error } => {
                write!(f, "invalid semantic shape for {exact}: {error}")
            }
            Self::MissingFacts(exact) => write!(f, "missing exact type facts for {exact}"),
            Self::DependencyIdentity { expected, actual } => {
                write!(f, "dependency facts for {expected} resolve to {actual}")
            }
            Self::ByValueCycle(exact) => write!(f, "by-value representation cycle at {exact}"),
            Self::EmptyCLayout(exact) => write!(f, "empty CLayout representation for {exact}"),
            Self::ZeroSizedCLayoutField { owner, field } => {
                write!(f, "CLayout {owner} contains zero-sized field type {field}")
            }
            Self::DuplicateVariant { owner, variant } => {
                write!(f, "duplicate enum variant {variant} in {owner}")
            }
            Self::VariantGc {
                owner,
                variant,
                expected,
                actual,
            } => write!(
                f,
                "enum {owner} variant {variant} GC fact {actual:?} differs from {expected:?}"
            ),
            Self::Mismatch {
                exact,
                expected_kind,
                expected_gc,
                actual,
            } => write!(
                f,
                "type facts for {exact} differ from semantic shape: expected {expected_kind:?}/{expected_gc:?}, actual {:?}/{:?}",
                actual.kind(),
                actual.gc()
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ExactTypeFactsSemanticError<E> {}
