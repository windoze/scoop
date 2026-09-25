use super::*;
use crate::{
    CanonicalConstValueKindV1, CheckedDeclarationAccessSourceV1, CheckedNominalInheritanceGraphV1,
    ConstPropertyDeclarationSourceV1, NominalSupportCallableSemanticAuthority,
    PropertyRepresentationV1, ProtectedPropertySemanticAuthority,
};
use scoop_identity::{PersistentTypeId, SignatureTypeKey, SourceDeclarationKind};

mod accessors;
mod errors;
pub use errors::*;

pub trait NominalSupportPropertySemanticAuthority<E>:
    ProtectedPropertySemanticAuthority<E> + NominalSupportCallableSemanticAuthority<E>
{
    fn const_source(
        &self,
        property: PersistentPropertyId,
    ) -> Result<&ConstPropertyDeclarationSourceV1, E>;
    fn canonical_const_value_type(
        &self,
        kind: CanonicalConstValueKindV1,
    ) -> Result<PersistentTypeId, E>;
}

/// Distinguishes concrete domain replay from retained generic source metadata.
/// Neither variant alone grants declaration selection or materialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominalSupportPropertyAccessProofV1 {
    ParamFreeDomains,
    GenericSourceMetadata,
}

#[derive(Clone, Copy, Debug)]
pub struct CheckedNominalSupportRuntimePropertySourceV1<'a> {
    declaration: PersistentPropertyId,
    payload: &'a NominalSourcePropertyPayloadV1,
    access: CheckedDeclarationAccessSourceV1<'a>,
    kind: SourceDeclarationKind,
    access_proof: NominalSupportPropertyAccessProofV1,
}
impl CheckedNominalSupportRuntimePropertySourceV1<'_> {
    pub const fn access_proof(&self) -> NominalSupportPropertyAccessProofV1 {
        self.access_proof
    }
    pub const fn declaration(&self) -> PersistentPropertyId {
        self.declaration
    }
    pub const fn payload(&self) -> &NominalSourcePropertyPayloadV1 {
        self.payload
    }
    pub const fn declaration_access(&self) -> CheckedDeclarationAccessSourceV1<'_> {
        self.access
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CheckedNominalSupportConstSourceV1<'a> {
    value: &'a ExportConstValueV1,
    access: CheckedDeclarationAccessSourceV1<'a>,
}
impl CheckedNominalSupportConstSourceV1<'_> {
    pub const fn value(&self) -> &ExportConstValueV1 {
        self.value
    }
    pub const fn declaration_access(&self) -> CheckedDeclarationAccessSourceV1<'_> {
        self.access
    }
}
#[derive(Clone, Copy, Debug)]
pub enum CheckedNominalSupportPropertySourceV1<'a> {
    Runtime(CheckedNominalSupportRuntimePropertySourceV1<'a>),
    Const(CheckedNominalSupportConstSourceV1<'a>),
}

impl NominalSupportPropertyInterfaceV1 {
    pub fn validate_source<'a, A: NominalSupportPropertySemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'a mut A,
    ) -> Result<CheckedNominalSupportPropertySourceV1<'a>, NominalSupportPropertySemanticError<E>>
    {
        use NominalSupportPropertySemanticError as Error;
        let owner = graph.source(self.owner()).ok_or(Error::Owner)?;
        let kind = owner.key.declaration_kind();
        match self.payload() {
            NominalSupportPropertyPayloadV1::Runtime { interface } => {
                let valid = match kind {
                    SourceDeclarationKind::Struct | SourceDeclarationKind::Enum => {
                        matches!(
                            interface.mutability(),
                            ProtectedPropertyMutabilityV1::ReadOnly
                        ) && interface.representation() == PropertyRepresentationV1::RuntimeAccessor
                    }
                    SourceDeclarationKind::Object => {
                        interface.representation() == PropertyRepresentationV1::RuntimeAccessor
                    }
                    SourceDeclarationKind::Class => {
                        interface.representation() != PropertyRepresentationV1::AbstractSlot
                            || authority
                                .source_nominal_modality(self.owner())
                                .map_err(Error::Foundation)?
                                == crate::NominalInheritanceModalityV1::Abstract
                    }
                    SourceDeclarationKind::Interface => true,
                    _ => false,
                };
                if !valid {
                    return Err(Error::Owner);
                }
                let generic_scope = self
                    .declaration_access()
                    .lexical_owners()
                    .iter()
                    .any(|owner| matches!(owner, SourceNominalId::GenericTemplate(_)));
                let validate = if generic_scope {
                    super::super::property::semantics::validate_source_template_contract
                } else {
                    super::super::property::semantics::validate_source_contract
                };
                let access = validate(
                    self.declaration(),
                    self.declaration_access(),
                    interface,
                    graph,
                    authority,
                )
                .map_err(Error::Runtime)?;
                Ok(CheckedNominalSupportPropertySourceV1::Runtime(
                    CheckedNominalSupportRuntimePropertySourceV1 {
                        declaration: self.declaration(),
                        payload: interface,
                        access,
                        kind,
                        access_proof: if generic_scope {
                            NominalSupportPropertyAccessProofV1::GenericSourceMetadata
                        } else {
                            NominalSupportPropertyAccessProofV1::ParamFreeDomains
                        },
                    },
                ))
            }
            NominalSupportPropertyPayloadV1::Const { value } => {
                if kind != SourceDeclarationKind::Object {
                    return Err(Error::Owner);
                }
                let source = authority
                    .const_source(self.declaration())
                    .map_err(Error::Foundation)?;
                let key = source.declaration();

                if PersistentPropertyId::from_source_declaration(key).ok()
                    != Some(self.declaration())
                    || key.origin() != owner.key.origin()
                    || key.package() != owner.key.package()
                    || key.owners().owners().split_last().map(|(_, outer)| outer)
                        != Some(owner.key.owners().owners())
                    || source.definition_origin() != value.definition_origin().origin()
                {
                    return Err(Error::ConstIdentity);
                }
                let expected = authority
                    .canonical_const_value_type(value.value().kind())
                    .map_err(Error::Foundation)?;
                if value.value_type() != &SignatureTypeKey::Nominal(expected) {
                    return Err(Error::ConstType);
                }
                let access = graph
                    .check_declaration_source(self.declaration_access(), key, authority)
                    .map_err(Error::Source)?;
                Ok(CheckedNominalSupportPropertySourceV1::Const(
                    CheckedNominalSupportConstSourceV1 { value, access },
                ))
            }
        }
    }
}
