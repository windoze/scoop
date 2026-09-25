use super::*;
use crate::{
    CheckedNominalInheritanceGraphV1, InheritanceGraphError, NominalInheritanceModalityV1,
    ProtectedCallableSemanticAuthority, ProtectedCallableSemanticError, SourceNominalId,
};
use scoop_identity::SourceDeclarationKind;

use std::fmt;

pub trait NominalSupportCallableSemanticAuthority<E>:
    ProtectedCallableSemanticAuthority<E>
{
    fn source_nominal_modality(
        &self,
        owner: SourceNominalId,
    ) -> Result<NominalInheritanceModalityV1, E>;
    fn source_enum_variant_key(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Result<&scoop_identity::EnumVariantIdentityKey, E>;
    fn source_enum_variant_shape(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Result<&crate::EnumSourceVariantV1, E>;
    fn source_enum_variant_field_key(
        &self,
        field: scoop_identity::PersistentEnumVariantFieldId,
    ) -> Result<&scoop_identity::EnumVariantFieldKey, E>;
    fn source_enum_variant_origin(
        &self,
        variant: scoop_identity::PersistentEnumVariantId,
    ) -> Result<&crate::ExportDefinitionSourceV1, E>;
}

/// Checked source support does not grant ordinary lookup, default expansion,
/// dispatch selection, or concrete materialization authority.
#[derive(Clone, Copy, Debug)]
pub struct CheckedNominalSupportCallableSourceV1<'a> {
    declaration: CallableTemplateOrigin,
    payload: &'a NominalSourceCallablePayloadV1,
    access: CheckedNominalSupportAccessSourceV1<'a>,
}
impl<'a> CheckedNominalSupportCallableSourceV1<'a> {
    pub const fn declaration(&self) -> CallableTemplateOrigin {
        self.declaration
    }
    pub const fn payload(&self) -> &'a NominalSourceCallablePayloadV1 {
        self.payload
    }
    pub const fn declaration_access(&self) -> CheckedNominalSupportAccessSourceV1<'a> {
        self.access
    }
}
impl NominalSupportCallableInterfaceV1 {
    pub fn validate_source<'a, A: NominalSupportCallableSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'a mut A,
    ) -> Result<CheckedNominalSupportCallableSourceV1<'a>, NominalSupportCallableSemanticError<E>>
    {
        validate(
            self.declaration(),
            self.payload(),
            self.declaration_access(),
            graph,
            authority,
        )
    }
}
impl NominalSupportConstructorInterfaceV1 {
    pub fn validate_source<'a, A: NominalSupportCallableSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'a mut A,
    ) -> Result<CheckedNominalSupportCallableSourceV1<'a>, NominalSupportCallableSemanticError<E>>
    {
        validate(
            CallableTemplateOrigin::Constructor(self.declaration()),
            self.payload(),
            self.declaration_access(),
            graph,
            authority,
        )
    }
}
fn validate<'a, A: NominalSupportCallableSemanticAuthority<E>, E>(
    declaration: CallableTemplateOrigin,
    payload: &'a NominalSourceCallablePayloadV1,
    source: &'a DeclarationAccessSourceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &'a mut A,
) -> Result<CheckedNominalSupportCallableSourceV1<'a>, NominalSupportCallableSemanticError<E>> {
    use NominalSupportCallableSemanticError as Error;
    let owner = graph.source(payload.owner()).ok_or(Error::Owner)?;
    if let CallableTemplateOrigin::VariantConstructor(variant) = declaration {
        let access = super::variants::validate(variant, payload, source, owner.key, authority)?;
        return Ok(CheckedNominalSupportCallableSourceV1 {
            declaration,
            payload,
            access: CheckedNominalSupportAccessSourceV1::Variant(access),
        });
    }

    let modality = authority
        .source_nominal_modality(payload.owner())
        .map_err(Error::Foundation)?;
    validate_owner(
        declaration,
        payload,
        source,
        owner.key.declaration_kind(),
        modality,
    )?;
    let access = super::super::semantics::validate_source_contract(
        declaration,
        payload,
        owner.key,
        source,
        graph,
        authority,
    )
    .map_err(Error::Signature)?;
    Ok(CheckedNominalSupportCallableSourceV1 {
        declaration,
        payload,
        access: CheckedNominalSupportAccessSourceV1::Declaration(access),
    })
}
fn validate_owner<E>(
    declaration: CallableTemplateOrigin,
    payload: &NominalSourceCallablePayloadV1,
    source: &DeclarationAccessSourceV1,
    kind: SourceDeclarationKind,
    modality: NominalInheritanceModalityV1,
) -> Result<(), NominalSupportCallableSemanticError<E>> {
    use NominalSupportCallableSemanticError as Error;
    let constructor = matches!(declaration, CallableTemplateOrigin::Constructor(_));
    if constructor
        && !matches!(
            kind,
            SourceDeclarationKind::Class | SourceDeclarationKind::Struct
        )
    {
        return Err(Error::Owner);
    }
    let callable = payload.modality();
    let valid = match kind {
        SourceDeclarationKind::Class => {
            modality != NominalInheritanceModalityV1::Interface
                && callable != CallableModalityV1::InterfaceDefault
                && (callable != CallableModalityV1::Abstract
                    || modality == NominalInheritanceModalityV1::Abstract)
        }
        SourceDeclarationKind::Interface => {
            modality == NominalInheritanceModalityV1::Interface
                && payload.type_parameters().is_empty()
                && match source.declared_visibility() {
                    DeclaredVisibilityV1::Private => {
                        callable == CallableModalityV1::Final && payload.slot_relations().is_empty()
                    }
                    DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Internal => matches!(
                        callable,
                        CallableModalityV1::Abstract | CallableModalityV1::InterfaceDefault
                    ),
                    DeclaredVisibilityV1::Protected => false,
                }
        }
        SourceDeclarationKind::Struct
        | SourceDeclarationKind::Enum
        | SourceDeclarationKind::Object => {
            modality == NominalInheritanceModalityV1::Final && callable == CallableModalityV1::Final
        }
        _ => false,
    };
    if valid { Ok(()) } else { Err(Error::Modality) }
}

#[derive(Debug)]
pub enum NominalSupportCallableSemanticError<E> {
    Foundation(E),
    Signature(ProtectedCallableSemanticError<E>),
    Source(InheritanceGraphError<E>),
    Owner,
    Modality,
    Variant(super::NominalSupportVariantError),
}
impl<E: fmt::Display> fmt::Display for NominalSupportCallableSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Variant(error) => error.fmt(f),
            Self::Owner => f.write_str("nominal source callable has an invalid source owner kind"),
            Self::Modality => {
                f.write_str("source callable modality is incompatible with its nominal owner")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for NominalSupportCallableSemanticError<E> {}
