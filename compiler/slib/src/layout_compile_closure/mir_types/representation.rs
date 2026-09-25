use scoop_hir as hir;
use scoop_identity::{
    GeneratedNominalKey, PersistentExactTypeId, PersistentFieldId, SignatureTypeKey,
};
use scoop_mir as mir;

use super::{
    Error, SharedMirTypeComponent as Component,
    validation::{Comparison, gc},
};

mod policies;

pub(super) fn validate(
    comparison: &mut Comparison<'_, '_>,
    source: &hir::NominalRepresentationSupportV1,
    record: &mir::ParamFreeMirTypeExportV1,
) -> Result<(), Error> {
    use hir::NominalRepresentationShapeV1 as Source;
    use mir::MirTypeRepresentationV1 as Repr;
    let exact = record.exact();
    let owner = source.owner();

    let declaration = comparison
        .source
        .metadata()
        .public
        .nominal_interfaces()
        .declaration(hir::SourceNominalId::Concrete(owner))
        .ok_or(Error::MissingDeclaration(owner))?;
    match (source.shape(), record.representation()) {
        (
            Source::Struct {
                fields,
                c_layout_policy,
            },
            Repr::Struct {
                fields: actual,
                c_layout,
                interior_mutable,
            },
        ) => {
            fields_match(
                comparison,
                exact,
                fields
                    .iter()
                    .map(|field| (field.field(), field.value_type())),
                actual,
            )?;
            Error::require(
                exact,
                Component::CLayout,
                policies::c_layout(*c_layout_policy) == *c_layout,
            )?;
            let hir::NominalSourceShapeV1::Struct(shape) = declaration.source_shape() else {
                return Err(Error::Mismatch {
                    exact,
                    component: Component::Representation,
                });
            };
            Error::require(
                exact,
                Component::InteriorMutable,
                shape.interior_mutable() == *interior_mutable,
            )
        }
        (Source::Enum { variants }, Repr::Enum { variants: actual }) => {
            variants_match(comparison, exact, variants, actual)
        }
        (
            Source::Class {
                declared_fields, ..
            },
            Repr::Class {
                kind,
                declared_fields: actual,
            },
        ) => {
            let expected = match declaration.declaration_details().modality() {
                hir::NominalInheritanceModalityV1::Final => mir::MirClassKindV1::Final,
                hir::NominalInheritanceModalityV1::Open => mir::MirClassKindV1::Open,
                hir::NominalInheritanceModalityV1::Abstract => mir::MirClassKindV1::Abstract,
                hir::NominalInheritanceModalityV1::Interface => {
                    return Err(Error::Mismatch {
                        exact,
                        component: Component::ClassKind,
                    });
                }
            };
            Error::require(exact, Component::ClassKind, *kind == expected)?;
            fields_match(
                comparison,
                exact,
                declared_fields
                    .iter()
                    .map(|field| (field.field(), field.value_type())),
                actual,
            )
        }
        (Source::Interface, Repr::Interface) => Ok(()),
        (Source::Intrinsic { representation }, Repr::Intrinsic(actual)) => Error::require(
            exact,
            Component::Representation,
            policies::intrinsic(representation.family()) == Some(*actual),
        ),
        (
            Source::Object {
                backing_class,
                declared_fields,
            },
            Repr::Object { backing },
        ) => {
            let expected = comparison.exact(*backing_class)?;
            Error::require(exact, Component::ObjectBacking, *backing == expected)?;
            let backing = comparison.require_type(expected)?;
            Error::require(
                expected,
                Component::Origin,
                backing.origin()
                    == &mir::MirTypeOriginV1::GeneratedNominal {
                        nominal: *backing_class,
                        role: GeneratedNominalKey::ObjectBackingClass { object: owner },
                    },
            )?;
            comparison.facts(exact, backing)?;
            comparison.inheritance(exact, backing)?;
            let Repr::ObjectBacking {
                declared_fields: actual,
            } = backing.representation()
            else {
                return Err(Error::Mismatch {
                    exact: expected,
                    component: Component::Representation,
                });
            };
            fields_match(
                comparison,
                expected,
                declared_fields
                    .iter()
                    .map(|field| (field.field(), field.value_type())),
                actual,
            )
        }
        _ => Err(Error::Mismatch {
            exact,
            component: Component::Representation,
        }),
    }
}

fn fields_match<'a>(
    comparison: &mut Comparison<'_, '_>,
    exact: PersistentExactTypeId,
    expected: impl ExactSizeIterator<Item = (PersistentFieldId, &'a SignatureTypeKey)>,
    actual: &[mir::MirRepresentationFieldV1],
) -> Result<(), Error> {
    Error::require(exact, Component::FieldCount, expected.len() == actual.len())?;

    for (index, ((field, signature), actual)) in expected.zip(actual).enumerate() {
        let value = comparison
            .source
            .metadata()
            .signature_exact_type(signature)?;
        Error::require(
            exact,
            Component::Field { index },
            field == actual.field && value == actual.value,
        )?;
    }
    Ok(())
}

fn variants_match(
    comparison: &mut Comparison<'_, '_>,
    exact: PersistentExactTypeId,
    expected: &[hir::EnumRepresentationVariantV1],
    actual: &[mir::MirRepresentationVariantV1],
) -> Result<(), Error> {
    Error::require(
        exact,
        Component::VariantCount,
        expected.len() == actual.len(),
    )?;

    for (variant, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        Error::require(
            exact,
            Component::Variant { index: variant },
            expected.variant() == actual.variant
                && gc(expected.gc()) == actual.gc
                && expected.fields().len() == actual.fields.len(),
        )?;

        for (index, (expected, actual)) in expected.fields().iter().zip(&actual.fields).enumerate()
        {
            let value = comparison
                .source
                .metadata()
                .signature_exact_type(expected.value_type())?;
            Error::require(
                exact,
                Component::VariantField { variant, index },
                expected.field() == actual.field && value == actual.value,
            )?;
        }
    }
    Ok(())
}
