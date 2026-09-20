use super::*;
use scoop_identity::SignatureTypeKey;

pub(super) fn validate(
    bound: &mut BoundNominalSourceContractsV1<'_, '_>,
    source: &NominalSourceContractV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let owner = source.owner();
    let path = WirePath::root();
    queries(bound.table.records().len(), meter)?;
    let key = bound.foundation.nominal_key(owner)?;
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
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
    meter.check_table_entries(source.type_parameters().binders().len() as u64, &path)?;
    for binder in source.type_parameters().binders() {
        meter.charge_nodes(1, &path)?;
        if let TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() {
            for bound_type in bounds
                .class()
                .into_iter()
                .chain(bounds.interfaces().values())
            {
                signature(&scope, bound_type, owner, bound, meter)?;
            }
        }
    }
    source
        .type_parameters()
        .validate_bound_semantics(None, bound)
        .map_err(|error| invalid(owner, error))?;
    meter.check_table_entries(source.supertypes().values().len() as u64, &path)?;
    let mut class_seen = false;
    for supertype in source.supertypes().values() {
        signature(&scope, supertype, owner, bound, meter)?;
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
    match source.source_shape() {
        NominalSourceShapeV1::Struct(shape) => {
            for field in shape.fields() {
                signature(&scope, field.value_type(), owner, bound, meter)?;
            }
        }
        NominalSourceShapeV1::Enum(shape) => {
            for variant in shape.variants() {
                for field in variant.fields() {
                    signature(&scope, field.value_type(), owner, bound, meter)?;
                }
            }
        }
        NominalSourceShapeV1::Class
        | NominalSourceShapeV1::Interface
        | NominalSourceShapeV1::Object(_) => {}
    }
    source
        .source_shape()
        .validate_semantics(
            owner,
            source.kind(),
            source.type_parameters(),
            &mut replay::ShapeAuthority { bound, meter },
        )
        .map_err(|error| match error {
            NominalSourceShapeSemanticError::StructField {
                error: StructSourceFieldSemanticError::Reference(error),
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
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    scope
        .validate_signature_semantics_metered(signature, bound, meter, &WirePath::root())
        .map_err(|error| match error {
            MeteredSignatureTypeSemanticError::Resource(error) => Error::Resource(error),
            MeteredSignatureTypeSemanticError::Semantic(error) => invalid(owner, error),
        })
}
