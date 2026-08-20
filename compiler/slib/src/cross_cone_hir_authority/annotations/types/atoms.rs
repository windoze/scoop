use scoop_hir::{
    AnnotationElementTypeV1, CanonicalAnnotationValueV1, CanonicalConstValueKindV1, IntegerKind,
};
use scoop_identity::{PersistentAnnotationId, PersistentEnumVariantId, SignatureTypeKey};
use std::collections::BTreeMap;

pub(in super::super) enum AnnotationAtomKind {
    Scalar(CanonicalConstValueKindV1),
    IntegralEnum {
        ty: SignatureTypeKey,
        kind: IntegerKind,
        variants: BTreeMap<PersistentEnumVariantId, u64>,
    },
    Annotation(PersistentAnnotationId),
}

pub(in super::super) enum AnnotationParameterKind {
    Atom(AnnotationAtomKind),
    Array(AnnotationAtomKind),
}

impl AnnotationParameterKind {
    pub(in super::super) fn nested(&self) -> Option<PersistentAnnotationId> {
        match self {
            Self::Atom(AnnotationAtomKind::Annotation(id))
            | Self::Array(AnnotationAtomKind::Annotation(id)) => Some(*id),
            _ => None,
        }
    }

    pub(in super::super) fn accepts(&self, value: &CanonicalAnnotationValueV1) -> bool {
        match (self, value) {
            (Self::Atom(kind), value) => kind.accepts(value),
            (
                Self::Array(kind),
                CanonicalAnnotationValueV1::Array {
                    element_type,
                    elements,
                },
            ) => {
                kind.matches_type(element_type)
                    && elements.iter().all(|element| kind.accepts(&element.value))
            }
            _ => false,
        }
    }
}

impl AnnotationAtomKind {
    fn matches_type(&self, ty: &AnnotationElementTypeV1) -> bool {
        match (self, ty) {
            (Self::Scalar(a), AnnotationElementTypeV1::Scalar(b)) => a == b,
            (Self::IntegralEnum { ty: a, .. }, AnnotationElementTypeV1::IntegralEnum(b)) => a == b,
            (Self::Annotation(a), AnnotationElementTypeV1::Annotation(b)) => a == b,
            _ => false,
        }
    }

    fn accepts(&self, value: &CanonicalAnnotationValueV1) -> bool {
        match (self, value) {
            (Self::Scalar(kind), CanonicalAnnotationValueV1::Scalar(value)) => {
                *kind == value.kind()
            }
            (
                Self::IntegralEnum { ty, kind, variants },
                CanonicalAnnotationValueV1::IntegralEnum {
                    value_type,
                    variant,
                    value,
                },
            ) => {
                ty == value_type
                    && *kind == value.kind()
                    && variants.get(variant) == Some(&value.raw_bits())
            }
            (Self::Annotation(id), CanonicalAnnotationValueV1::Annotation { annotation, .. }) => {
                id == annotation
            }
            _ => false,
        }
    }
}
