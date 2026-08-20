use std::fmt;

use scoop_identity::{
    CborIdentityRecord, DeclarationScope, DefinitionOwnerChain, PersistentConstructorId,
    SourceDeclarationKey, SourceDeclarationSite,
};

use super::HirSourceConstructorIdentity;
use crate::{
    ConstructorParameter, HirSignatureBinder, HirSignatureTypeMapper, HirSignatureTypeMappingError,
    HirSourceNominalIdentity, HirTypeIdentityInputs, TypeParamDecl,
};

pub fn derive_source_constructor_identity(
    type_inputs: HirTypeIdentityInputs<'_>,
    owner: &HirSourceNominalIdentity,
    type_parameters: &[TypeParamDecl],
    parameters: &[ConstructorParameter],
) -> Result<HirSourceConstructorIdentity, HirSourceConstructorIdentityError> {
    let binders = type_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            Ok(HirSignatureBinder {
                parameter: parameter.id,
                depth: 0,
                index: u32::try_from(index)
                    .map_err(|_| HirSourceConstructorIdentityError::TooManyTypeParameters)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mapper = HirSignatureTypeMapper::new(type_inputs);
    let parameters = parameters
        .iter()
        .map(|parameter| mapper.map(parameter.ty, &binders))
        .collect::<Result<Vec<_>, _>>()
        .map_err(HirSourceConstructorIdentityError::InvalidSignatureType)?;
    let mut owners = owner.declaration().owners().owners().to_vec();
    owners.push(owner.definition_owner());
    let site = SourceDeclarationSite::new(
        owner.declaration().origin(),
        owner.declaration().package().clone(),
        DefinitionOwnerChain::from_outer_to_inner(owners),
        DeclarationScope::ConeWide,
    )
    .map_err(HirSourceConstructorIdentityError::InvalidSite)?;
    CborIdentityRecord::<PersistentConstructorId, SourceDeclarationKey>::from_key(
        SourceDeclarationKey::constructor(site, parameters),
    )
    .map_err(HirSourceConstructorIdentityError::InvalidIdentity)
}

#[derive(Debug)]
pub enum HirSourceConstructorIdentityError {
    TooManyTypeParameters,
    InvalidSignatureType(HirSignatureTypeMappingError),
    InvalidSite(scoop_identity::SourceDeclarationKeyError),
    InvalidIdentity(scoop_identity::SourceDeclarationIdentityError),
}

impl fmt::Display for HirSourceConstructorIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyTypeParameters => {
                formatter.write_str("type parameter count exceeds the identity schema")
            }
            Self::InvalidSignatureType(error) => error.fmt(formatter),
            Self::InvalidSite(error) => error.fmt(formatter),
            Self::InvalidIdentity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for HirSourceConstructorIdentityError {}
