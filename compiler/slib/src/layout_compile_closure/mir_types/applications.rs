//! Joins actual MIR applications to the original shared declaration and facts.

use scoop_hir as hir;
use scoop_identity::{
    ExactTypeKey, PersistentExactTypeId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationKind,
};
use scoop_mir as mir;

use super::{
    Error, SharedMirTypeComponent as Component,
    validation::{Comparison, gc},
};

pub(super) fn validate(
    comparison: &mut Comparison<'_, '_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    key: &ExactTypeKey,
) -> Result<(), Error> {
    let exact = PersistentExactTypeId::from_key(key)
        .map_err(|error| hir::SharedTypeMetadataError::Key(error.to_string()))?;
    let record = comparison.require_type(exact)?;
    let ExactTypeKey::NominalApplication { origin, arguments } = key else {
        return Err(Error::Mismatch {
            exact,
            component: Component::Origin,
        });
    };
    Error::require(
        exact,
        Component::Origin,
        record.origin() == &mir::MirTypeOriginV1::NominalApplication(*origin),
    )?;
    comparison.facts(exact, record)?;
    let metadata = comparison.source.metadata();
    let declaration_key = metadata
        .identities
        .canonical_key::<_, SourceDeclarationKey>(*origin)
        .map_err(hir::SharedTypeMetadataError::from)?;
    let provider = declaration_key.origin();
    let public = if provider == metadata.provider {
        metadata.public
    } else {
        dependencies
            .iter()
            .find(|source| source.provider() == provider)
            .ok_or(hir::SharedTypeMetadataError::MissingProvider(provider))?
            .metadata()
            .public
    };
    let declaration = public
        .nominal_interfaces()
        .declaration(hir::SourceNominalId::GenericTemplate(*origin))
        .ok_or(hir::SharedTypeMetadataError::MissingGenericNominal(*origin))?;
    let bindings = [arguments.as_slice().to_vec()];
    let resolve = |signature: &SignatureTypeKey| -> Result<_, Error> {
        Ok(comparison.source.signature_exact_type_with_bindings(
            signature,
            &bindings,
            dependencies,
        )?)
    };
    use hir::NominalSourceShapeV1 as Source;
    use mir::MirTypeRepresentationV1 as Repr;
    let fields = match (declaration.source_shape(), record.representation()) {
        (
            Source::Struct(source),
            Repr::Struct {
                fields,
                c_layout,
                interior_mutable,
            },
        ) => {
            Error::require(
                exact,
                Component::CLayout,
                *c_layout == super::representation::policies::c_layout(source.c_layout_policy()),
            )?;
            Error::require(
                exact,
                Component::InteriorMutable,
                *interior_mutable == source.interior_mutable(),
            )?;
            Some((source.fields(), fields.as_slice()))
        }
        (
            Source::Class(source),
            Repr::Class {
                kind,
                declared_fields,
                release_policy,
            },
        ) => {
            let matches = matches!(
                (declaration.declaration_details().modality(), kind),
                (
                    hir::NominalInheritanceModalityV1::Final,
                    mir::MirClassKindV1::Final
                ) | (
                    hir::NominalInheritanceModalityV1::Open,
                    mir::MirClassKindV1::Open
                ) | (
                    hir::NominalInheritanceModalityV1::Abstract,
                    mir::MirClassKindV1::Abstract
                )
            );
            Error::require(exact, Component::ClassKind, matches)?;
            Error::require(
                exact,
                Component::ReleasePolicy,
                *release_policy
                    == super::representation::policies::release(
                        declaration.declaration_details().release_policy(),
                        exact,
                    ),
            )?;
            Some((source.fields(), declared_fields.as_slice()))
        }
        (Source::Enum(source), Repr::Enum { variants }) => {
            Error::require(
                exact,
                Component::VariantCount,
                source.variants().len() == variants.len(),
            )?;
            for (index, (expected, actual)) in source.variants().iter().zip(variants).enumerate() {
                Error::require(
                    exact,
                    Component::Variant { index },
                    expected.variant() == actual.variant
                        && expected.fields().len() == actual.fields.len(),
                )?;
                let mut references = false;
                for (field, (expected, actual)) in
                    expected.fields().iter().zip(&actual.fields).enumerate()
                {
                    Error::require(
                        exact,
                        Component::VariantField {
                            variant: index,
                            index: field,
                        },
                        expected.field() == actual.field
                            && resolve(expected.value_type())? == actual.value,
                    )?;
                    let fact = comparison
                        .source
                        .facts()
                        .get_checked(actual.value)
                        .or_else(|| {
                            dependencies
                                .iter()
                                .find_map(|source| source.facts().get_checked(actual.value))
                        })
                        .ok_or(Error::MissingFacts(actual.value))?;
                    references |=
                        gc(fact.record().gc()) == mir::MirGcKindV1::ContainsManagedReferences;
                }
                Error::require(
                    exact,
                    Component::Variant { index },
                    (actual.gc == mir::MirGcKindV1::ContainsManagedReferences) == references,
                )?;
            }
            None
        }
        (Source::Interface, Repr::Interface) => None,
        (Source::Intrinsic(source), Repr::InlineArray { element }) => {
            Error::require(
                exact,
                Component::Representation,
                matches!(
                    source.family(),
                    hir::IntrinsicTypeKind::Array | hir::IntrinsicTypeKind::MutableArray
                ) && arguments.as_slice() == [*element],
            )?;
            None
        }
        _ => {
            return Err(Error::Mismatch {
                exact,
                component: Component::Representation,
            });
        }
    };
    if let Some((expected, actual)) = fields {
        Error::require(exact, Component::FieldCount, expected.len() == actual.len())?;
        for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
            Error::require(
                exact,
                Component::Field { index },
                expected.field() == actual.field && resolve(expected.value_type())? == actual.value,
            )?;
        }
    }
    let mut base = mir::MirBaseClassV1::None;
    let mut interfaces = Vec::new();
    for signature in declaration.exact_supertypes().values() {
        let parent = resolve(signature)?;
        let key = metadata
            .identities
            .canonical_key::<_, ExactTypeKey>(parent)
            .map_err(hir::SharedTypeMetadataError::from)?;
        let declaration = match key.as_ref() {
            ExactTypeKey::Nominal(owner) => metadata
                .identities
                .canonical_key::<_, SourceDeclarationKey>(*owner),
            ExactTypeKey::NominalApplication { origin, .. } => {
                metadata
                    .identities
                    .canonical_key::<_, SourceDeclarationKey>(*origin)
            }
            _ => {
                return Err(Error::Mismatch {
                    exact,
                    component: Component::Base,
                });
            }
        }
        .map_err(hir::SharedTypeMetadataError::from)?;
        match declaration.declaration_kind() {
            SourceDeclarationKind::Class => base = mir::MirBaseClassV1::Base(parent),
            SourceDeclarationKind::Interface => interfaces.push(parent),
            _ => {
                return Err(Error::Mismatch {
                    exact,
                    component: Component::Base,
                });
            }
        }
    }
    interfaces.sort_unstable();
    Error::require(
        exact,
        Component::Base,
        record.base_and_interfaces().base == base,
    )?;
    Error::require(
        exact,
        Component::Interfaces,
        record.base_and_interfaces().interfaces == interfaces,
    )
}
