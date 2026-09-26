//! Recursive typed pattern-matrix exhaustiveness checking.
//!
//! Guarded rows are excluded before specialization. Sum constructors are
//! visited in declaration order, product fields in source order, Boolean in
//! `false`/`true` order, and integers in exact mathematical order. Open
//! domains use one symbolic value not named by a literal in the matrix.

use std::collections::{BTreeMap, BTreeSet};

use scoop_ast::Span;
use scoop_hir as hir;

use crate::{Lowerer, Type};

use self::{
    constructors::{Constructor, ConstructorSpace, Witness},
    integers::{IntegerDomain, first_missing_integer_ordinal, integer_domain, integer_pattern_raw},
};

mod constructors;
mod integers;

type Matrix = Vec<Vec<hir::Pattern>>;

impl Lowerer {
    pub(crate) fn prove_exhaustiveness(
        &mut self,
        span: Span,
        subject_ty: hir::TypeId,
        arms: &[hir::WhenArm],
    ) -> Option<hir::ExhaustivenessProof> {
        let matrix: Matrix = arms
            .iter()
            .filter(|arm| arm.guard.is_none())
            .map(|arm| vec![arm.pattern.clone()])
            .collect();

        if let Some(witness) = self.missing_witness(&[subject_ty], &matrix) {
            let witness = witness
                .into_iter()
                .next()
                .expect("a non-exhaustive one-column matrix has one witness")
                .render();
            let guard_note = arms
                .iter()
                .any(|arm| arm.guard.is_some())
                .then_some("; guarded arms do not contribute to exhaustiveness");
            self.error(
                span,
                format!(
                    "non-exhaustive when: missing pattern {witness}{}",
                    guard_note.unwrap_or_default(),
                ),
            );
            return None;
        }

        if arms
            .iter()
            .any(|arm| arm.guard.is_none() && super::is_irrefutable(&arm.pattern))
        {
            return Some(hir::ExhaustivenessProof::IrrefutableArm { subject_ty });
        }
        match self.types[subject_ty] {
            Type::Enum(application) => Some(hir::ExhaustivenessProof::EnumPatternMatrix {
                subject_ty,
                application,
            }),
            Type::Tuple(_) | Type::Struct(_) | Type::Integer(_) => {
                Some(hir::ExhaustivenessProof::PatternMatrix { subject_ty })
            }
            _ => {
                unreachable!("a source pattern when has an enum, tuple, struct, or integer subject")
            }
        }
    }

    fn missing_witness(&mut self, types: &[hir::TypeId], matrix: &Matrix) -> Option<Vec<Witness>> {
        if types.is_empty() {
            return matrix.is_empty().then(Vec::new);
        }
        debug_assert!(matrix.iter().all(|row| row.len() == types.len()));

        // A recursively irrefutable row covers the complete remaining
        // product. Besides avoiding needless specialization, this is the
        // terminating case for wildcard payloads of recursive constructors.
        if matrix
            .iter()
            .any(|row| row.iter().all(super::is_irrefutable))
        {
            return None;
        }

        let subject_ty = types[0];
        if let Some(domain) = integer_domain(&self.types[subject_ty]) {
            return self.missing_integer_witness(types, matrix, domain);
        }
        match self.constructor_space(subject_ty, matrix) {
            ConstructorSpace::Closed(constructors) => {
                for constructor in constructors {
                    if let Some(witness) = self.missing_for_constructor(types, matrix, constructor)
                    {
                        return Some(witness);
                    }
                }
                None
            }
            ConstructorSpace::Open { prefix, remainder } => {
                for constructor in prefix {
                    if let Some(witness) = self.missing_for_constructor(types, matrix, constructor)
                    {
                        return Some(witness);
                    }
                }

                // Every value represented by `remainder` sees exactly the
                // wildcard-head rows. If those rows cover the remaining
                // columns, literal-specific rows can only add coverage, so
                // the entire open domain is covered.
                self.missing_for_constructor(types, matrix, remainder)
            }
        }
    }

    /// Split one fixed-width integer column into the singleton literals that
    /// actually occur in the matrix and one symbolic `OtherInteger`
    /// partition. Rows with a wildcard head belong to every partition;
    /// literal rows belong only to their exact raw-bit singleton.
    ///
    /// Grouping the rows once is important for a complete Int16/UInt16
    /// matrix: specializing the original matrix once per literal would turn
    /// 65,536 source rows into quadratic work. This path is proportional to
    /// the rows that actually exist and never enumerates absent domain values.
    fn missing_integer_witness(
        &mut self,
        types: &[hir::TypeId],
        matrix: &Matrix,
        domain: IntegerDomain,
    ) -> Option<Vec<Witness>> {
        let mut singleton_rows: BTreeMap<u128, Matrix> = BTreeMap::new();
        let mut other_rows = Matrix::new();

        for row in matrix {
            let tail = row[1..].to_vec();
            if super::is_irrefutable(&row[0]) {
                other_rows.push(tail);
            } else if let Some(raw) = integer_pattern_raw(&row[0], domain) {
                singleton_rows
                    .entry(domain.ordinal(raw))
                    .or_default()
                    .push(tail);
            } else {
                unreachable!(
                    "a checked integer matrix contains only integer literals and wildcards"
                );
            }
        }

        // `OtherInteger(kind, excluded)` contains every value without an
        // explicit singleton. Its rows are exactly the wildcard-head rows.
        // If they already cover the tail product, they also cover every
        // singleton and the entire integer column is exhaustive.
        let other_witness = self.missing_witness(&types[1..], &other_rows)?;
        let first_other = first_missing_integer_ordinal(domain, singleton_rows.keys().copied());

        // Visit only source-observed singleton partitions before the first
        // real value in `OtherInteger`, preserving mathematical witness order.
        for (ordinal, literal_rows) in singleton_rows {
            if first_other.is_some_and(|first_other| ordinal > first_other) {
                break;
            }
            let mut specialized = other_rows.clone();
            specialized.extend(literal_rows);
            if let Some(tail) = self.missing_witness(&types[1..], &specialized) {
                return Some(integer_witness(domain, ordinal, tail));
            }
        }

        first_other.map(|ordinal| integer_witness(domain, ordinal, other_witness))
    }

    fn missing_for_constructor(
        &mut self,
        types: &[hir::TypeId],
        matrix: &Matrix,
        constructor: Constructor,
    ) -> Option<Vec<Witness>> {
        let field_types = constructor.field_types().to_vec();
        let specialized = self.specialize_matrix(matrix, types[0], &constructor);
        let mut specialized_types = field_types;
        specialized_types.extend_from_slice(&types[1..]);
        let mut witness = self.missing_witness(&specialized_types, &specialized)?;
        let tail = witness.split_off(constructor.arity());
        let head = constructor.into_witness(witness);
        let mut result = Vec::with_capacity(1 + tail.len());
        result.push(head);
        result.extend(tail);
        Some(result)
    }

    fn constructor_space(&mut self, ty: hir::TypeId, matrix: &Matrix) -> ConstructorSpace {
        match self.types[ty].clone() {
            Type::ImportedStruct(structure) => {
                ConstructorSpace::Closed(vec![Constructor::Struct {
                    name: structure.declaration.name().to_owned(),
                    field_names: structure
                        .fields
                        .iter()
                        .map(|field| field.name.clone())
                        .collect(),
                    field_types: structure.fields.iter().map(|field| field.ty).collect(),
                }])
            }
            Type::ImportedEnum(enumeration) => ConstructorSpace::Closed(
                enumeration
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| Constructor::EnumVariant {
                        variant: index as u32,
                        name: variant.name.clone(),
                        style: match variant.style {
                            hir::EnumSourceVariantStyleV1::Unit => hir::VariantStyle::Unit,
                            hir::EnumSourceVariantStyleV1::Positional => {
                                hir::VariantStyle::Positional
                            }
                            hir::EnumSourceVariantStyleV1::Named => hir::VariantStyle::Named,
                            hir::EnumSourceVariantStyleV1::Constructor => {
                                hir::VariantStyle::Constructor
                            }
                        },
                        field_names: variant
                            .fields
                            .iter()
                            .map(|field| field.name.clone())
                            .collect(),
                        field_types: variant.fields.iter().map(|field| field.ty).collect(),
                    })
                    .collect(),
            ),
            Type::Enum(application) => {
                let application_value = self.enum_applications[application].clone();
                let enum_id = application_value.template;
                let variants = self.enums[enum_id].variants.clone();
                ConstructorSpace::Closed(
                    variants
                        .into_iter()
                        .enumerate()
                        .map(|(index, variant)| Constructor::EnumVariant {
                            variant: index as u32,
                            name: variant.name,
                            style: variant.style,
                            field_names: variant
                                .fields
                                .iter()
                                .map(|field| field.name.clone())
                                .collect(),
                            field_types: self.variant_field_types(
                                enum_id,
                                index as u32,
                                &application_value.arguments,
                            ),
                        })
                        .collect(),
                )
            }
            Type::Tuple(field_types) => {
                ConstructorSpace::Closed(vec![Constructor::Tuple { field_types }])
            }
            Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                let declaration = self.structs[application_value.template].clone();
                let field_names = declaration
                    .semantic_fields()
                    .iter()
                    .map(|field| field.name.clone())
                    .collect();
                let field_types = declaration
                    .semantic_fields()
                    .iter()
                    .map(|field| self.instantiate_ty(field.ty, &application_value.arguments))
                    .collect();
                ConstructorSpace::Closed(vec![Constructor::Struct {
                    name: declaration.name,
                    field_names,
                    field_types,
                }])
            }
            Type::Boolean => ConstructorSpace::Closed(vec![
                Constructor::Boolean(false),
                Constructor::Boolean(true),
            ]),
            Type::Unit => ConstructorSpace::Closed(vec![Constructor::Unit]),
            Type::String => self.string_constructor_space(matrix),
            Type::Integer(_) => {
                unreachable!("integer columns use symbolic singleton/other partitioning")
            }
            Type::Class(_)
            | Type::Interface(_)
            | Type::Any
            | Type::Function(_)
            | Type::Ptr(_)
            | Type::FunPtr(_)
            | Type::Param(_) => ConstructorSpace::Open {
                prefix: Vec::new(),
                remainder: Constructor::Opaque,
            },
        }
    }

    fn string_constructor_space(&self, matrix: &Matrix) -> ConstructorSpace {
        let literals: BTreeSet<String> = matrix
            .iter()
            .filter_map(|row| string_pattern_value(&row[0]).map(str::to_owned))
            .collect();
        let mut prefix = Vec::new();
        let mut candidate = String::new();
        while literals.contains(&candidate) {
            prefix.push(Constructor::String {
                value: candidate,
                remainder: false,
            });
            candidate = if prefix.len() == 1 {
                "a".to_string()
            } else {
                format!(
                    "{}a",
                    prefix
                        .last()
                        .expect("a string prefix exists")
                        .string_value()
                )
            };
        }
        ConstructorSpace::Open {
            prefix,
            remainder: Constructor::String {
                value: candidate,
                remainder: true,
            },
        }
    }

    fn specialize_matrix(
        &self,
        matrix: &Matrix,
        ty: hir::TypeId,
        constructor: &Constructor,
    ) -> Matrix {
        matrix
            .iter()
            .filter_map(|row| {
                let mut head = self.specialize_pattern(&row[0], ty, constructor)?;
                head.extend(row[1..].iter().cloned());
                Some(head)
            })
            .collect()
    }

    fn specialize_pattern(
        &self,
        pattern: &hir::Pattern,
        ty: hir::TypeId,
        constructor: &Constructor,
    ) -> Option<Vec<hir::Pattern>> {
        if matches!(
            pattern,
            hir::Pattern::Binding { .. } | hir::Pattern::Wildcard
        ) {
            return Some(wildcards(constructor.arity()));
        }
        match (constructor, pattern) {
            (
                Constructor::EnumVariant {
                    variant,
                    field_types,
                    ..
                },
                hir::Pattern::Variant {
                    variant: pattern_variant,
                    fields,
                    ..
                },
            ) if variant == pattern_variant => Some(normalize_fields(field_types.len(), fields)),
            (Constructor::Tuple { field_types }, hir::Pattern::Tuple(elements)) => {
                debug_assert_eq!(elements.len(), field_types.len());
                Some(elements.clone())
            }
            (Constructor::Struct { field_types, .. }, hir::Pattern::Struct { fields, .. }) => {
                Some(normalize_fields(field_types.len(), fields))
            }
            (Constructor::Boolean(expected), hir::Pattern::Literal { value, .. }) if matches!(value.kind, hir::ExprKind::BoolLiteral(actual) if actual == *expected) => {
                Some(Vec::new())
            }
            (Constructor::Unit, hir::Pattern::Literal { value, .. })
                if matches!(value.kind, hir::ExprKind::UnitLiteral) =>
            {
                Some(Vec::new())
            }
            (
                Constructor::String {
                    value: expected,
                    remainder: false,
                },
                hir::Pattern::Literal { .. },
            ) if string_pattern_value(pattern) == Some(expected.as_str()) => Some(Vec::new()),
            (
                Constructor::String {
                    remainder: true, ..
                }
                | Constructor::Opaque,
                _,
            ) => None,
            _ => {
                debug_assert!(
                    !matches!(pattern, hir::Pattern::Literal { .. })
                        || literal_matches_type(pattern, &self.types[ty]),
                    "a checked literal pattern matches its recursive subject type",
                );
                None
            }
        }
    }
}

fn integer_witness(domain: IntegerDomain, ordinal: u128, tail: Vec<Witness>) -> Vec<Witness> {
    let mut result = Vec::with_capacity(1 + tail.len());
    result.push(Witness::Integer {
        domain,
        raw: domain.raw_from_ordinal(ordinal),
    });
    result.extend(tail);
    result
}

fn string_pattern_value(pattern: &hir::Pattern) -> Option<&str> {
    let hir::Pattern::Literal { value, .. } = pattern else {
        return None;
    };
    let hir::ExprKind::StringLiteral { value, .. } = &value.kind else {
        return None;
    };
    Some(value)
}

fn literal_matches_type(pattern: &hir::Pattern, ty: &Type) -> bool {
    let hir::Pattern::Literal { value, .. } = pattern else {
        return false;
    };
    matches!(
        (&value.kind, ty),
        (hir::ExprKind::BoolLiteral(_), Type::Boolean)
            | (hir::ExprKind::UnitLiteral, Type::Unit)
            | (hir::ExprKind::StringLiteral { .. }, Type::String)
            | (hir::ExprKind::IntegerLiteral(_), Type::Integer(_))
    )
}

fn wildcards(count: usize) -> Vec<hir::Pattern> {
    (0..count).map(|_| hir::Pattern::Wildcard).collect()
}

fn normalize_fields(count: usize, fields: &[(u32, hir::Pattern)]) -> Vec<hir::Pattern> {
    let mut normalized = wildcards(count);
    for (index, pattern) in fields {
        normalized[*index as usize] = pattern.clone();
    }
    normalized
}
