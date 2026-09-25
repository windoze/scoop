use super::*;
use crate::{
    AccessDomainSemanticError, CheckedDeclarationAccessSourceV1, CheckedNominalInheritanceGraphV1,
    ReplayedDeclarationAccessDomainsV1, SourceNominalId,
};
use scoop_identity::{
    EnumVariantIdentityKey, PersistentEnumVariantId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationKind,
};
use std::fmt;

mod fields;

#[derive(Clone, Copy, Debug)]
pub enum CheckedNominalSupportAccessSourceV1<'a> {
    Declaration(CheckedDeclarationAccessSourceV1<'a>),
    Variant(CheckedEnumVariantAccessSourceV1<'a>),
}
impl<'s> CheckedNominalSupportAccessSourceV1<'s> {
    pub const fn source(&self) -> &'s DeclarationAccessSourceV1 {
        match self {
            Self::Declaration(source) => source.source(),
            Self::Variant(source) => source.source(),
        }
    }
    pub fn replay<'g, 'a>(
        &self,
        graph: &'g CheckedNominalInheritanceGraphV1<'a>,
    ) -> Result<ReplayedDeclarationAccessDomainsV1<'g, 'a>, AccessDomainSemanticError> {
        match self {
            Self::Declaration(source) => graph.replay_declaration_access(*source),
            Self::Variant(source) => graph.replay_variant_access(*source),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CheckedEnumVariantAccessSourceV1<'a> {
    source: &'a DeclarationAccessSourceV1,
    variant: &'a EnumVariantIdentityKey,
}
impl<'s> CheckedEnumVariantAccessSourceV1<'s> {
    pub const fn source(&self) -> &'s DeclarationAccessSourceV1 {
        self.source
    }
    pub const fn variant(&self) -> &EnumVariantIdentityKey {
        self.variant
    }
}

pub(super) fn validate<'a, A: NominalSupportCallableSemanticAuthority<E>, E>(
    variant: PersistentEnumVariantId,
    payload: &NominalSourceCallablePayloadV1,
    source: &'a DeclarationAccessSourceV1,
    owner: &SourceDeclarationKey,
    authority: &'a mut A,
) -> Result<CheckedEnumVariantAccessSourceV1<'a>, NominalSupportCallableSemanticError<E>> {
    use NominalSupportCallableSemanticError as Error;
    use NominalSupportVariantError as VariantError;
    if owner.declaration_kind() != SourceDeclarationKind::Enum
        || authority
            .source_nominal_modality(payload.owner())
            .map_err(Error::Foundation)?
            != crate::NominalInheritanceModalityV1::Final
    {
        return Err(Error::Owner);
    }
    super::super::semantics::signature::validate_types(
        payload,
        owner.duplicate_signature().type_parameter_count(),
        authority,
    )
    .map_err(Error::Signature)?;
    let key = authority
        .source_enum_variant_key(variant)
        .map_err(Error::Foundation)?;

    let fail = Error::Variant;

    if PersistentEnumVariantId::from_key(key).ok() != Some(variant)
        || key.source_owner() != Some(payload.owner())
    {
        return Err(fail(VariantError::Identity));
    }
    let owner_source = authority
        .nominal_access_source(payload.owner())
        .map_err(Error::Foundation)?;
    let (last, outer) = source
        .lexical_owners()
        .split_last()
        .ok_or_else(|| fail(VariantError::Access))?;

    if source.declared_visibility() != DeclaredVisibilityV1::Public
        || *last != payload.owner()
        || outer != owner_source.lexical_owners()
        || source.definition_origin().origin().source()
            != owner_source.definition_origin().origin().source()
        || source.definition_origin()
            != authority
                .source_enum_variant_origin(variant)
                .map_err(Error::Foundation)?
    {
        return Err(fail(VariantError::Access));
    }
    authority
        .validate_definition_source(source.definition_origin())
        .map_err(Error::Foundation)?;
    let shape = authority
        .source_enum_variant_shape(variant)
        .map_err(Error::Foundation)?;
    fields::validate(variant, payload, shape, authority)?;
    let result_matches = match (payload.owner(), payload.result()) {
        (SourceNominalId::Concrete(owner), SignatureTypeKey::Nominal(result)) => owner == *result,
        (SourceNominalId::GenericTemplate(owner), SignatureTypeKey::NominalApplication { origin, arguments }) => owner == *origin && arguments.as_slice().iter().enumerate().all(|(index, value)| matches!(value, SignatureTypeKey::Binder { depth: 0, index: actual } if *actual as usize == index)),
        _ => false,
    };
    if !result_matches {
        return Err(fail(VariantError::Result));
    }
    Ok(CheckedEnumVariantAccessSourceV1 {
        source,
        variant: key,
    })
}

#[derive(Debug)]
pub enum NominalSupportVariantError {
    Resource(scoop_wire::WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Identity,
    Access,
    Parameters,
    Result,
}
impl fmt::Display for NominalSupportVariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Identity => {
                f.write_str("variant support identity differs from its source enum owner")
            }
            Self::Access => f.write_str(
                "variant support requires its exact public source origin and enum owner chain",
            ),
            Self::Parameters => {
                f.write_str("variant source parameters differ from its canonical field sequence")
            }
            Self::Result => {
                f.write_str("variant source result differs from its enum owner application")
            }
        }
    }
}
impl std::error::Error for NominalSupportVariantError {}
