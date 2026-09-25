use super::*;
use scoop_identity::SignatureTypeKey;

pub(super) fn validate(
    bound: &mut BoundNominalSourceContractsV1<'_, '_>,
    source: &NominalSourceContractV1,
) -> Result<(), Error> {
    let owner = source.owner();

    let key = bound.foundation.nominal_key(owner)?;

    let header = replay::shape(key)?;
    if header.kind() != source.kind()
        || header.type_parameter_arity() as usize != source.type_parameters().binders().len()
    {
        return Err(invalid(
            owner,
            "source kind or own binder arity disagrees with identity",
        ));
    }
    let scope = source.type_parameters().signature_scope(None);

    for binder in source.type_parameters().binders() {
        if let TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() {
            for bound_type in bounds
                .class()
                .into_iter()
                .chain(bounds.interfaces().values())
            {
                signature(&scope, bound_type, owner, bound)?;
            }
        }
    }
    source
        .type_parameters()
        .validate_bound_semantics(None, bound)
        .map_err(|error| invalid(owner, error))?;

    let mut class_seen = false;
    for supertype in source.supertypes().values() {
        signature(&scope, supertype, owner, bound)?;
        let kind = match supertype {
            SignatureTypeKey::Nominal(id) => bound.concrete_nominal_shape(*id)?.kind(),
            SignatureTypeKey::NominalApplication { origin, .. } => {
                bound.generic_nominal_shape(*origin)?.kind()
            }
            _ => {
                return Err(invalid(
                    owner,
                    "supertype must be a nominal class or interface",
                ));
            }
        };
        match kind {
            PublicNominalKindV1::Interface => {}
            PublicNominalKindV1::Class
                if !class_seen
                    && matches!(
                        source.kind(),
                        PublicNominalKindV1::Class | PublicNominalKindV1::Object
                    ) =>
            {
                class_seen = true;
            }
            _ => {
                return Err(invalid(
                    owner,
                    "invalid nominal supertype kind or repeated class base",
                ));
            }
        }
    }
    for field in source.source_shape().declared_fields() {
        signature(&scope, field.value_type(), owner, bound)?;
    }
    if let NominalSourceShapeV1::Enum(shape) = source.source_shape() {
        for variant in shape.variants() {
            for field in variant.fields() {
                signature(&scope, field.value_type(), owner, bound)?;
            }
        }
    }
    source
        .source_shape()
        .validate_semantics(owner, source.kind(), source.type_parameters(), bound)
        .map_err(|error| match error {
            NominalSourceShapeSemanticError::NominalField {
                error: NominalSourceFieldSemanticError::Reference(error),
                ..
            }
            | NominalSourceShapeSemanticError::EnumVariant {
                error: EnumSourceVariantSemanticError::Reference(error),
                ..
            }
            | NominalSourceShapeSemanticError::EnumVariant {
                error:
                    EnumSourceVariantSemanticError::Field {
                        error: EnumSourceFieldSemanticError::Reference(error),
                        ..
                    },
                ..
            }
            | NominalSourceShapeSemanticError::ObjectValue(
                ObjectSourceShapeSemanticError::Reference(error),
            ) => error,
            other => invalid(owner, other),
        })
}

fn signature(
    scope: &SignatureBinderScopeV1,
    signature: &SignatureTypeKey,
    owner: SourceNominalId,
    bound: &mut BoundNominalSourceContractsV1<'_, '_>,
) -> Result<(), Error> {
    scope
        .validate_signature_semantics(signature, bound)
        .map_err(|error| match error {
            SignatureTypeSemanticError::Allocation(error) => Error::Resource(error),
            error => invalid(owner, error),
        })
}
