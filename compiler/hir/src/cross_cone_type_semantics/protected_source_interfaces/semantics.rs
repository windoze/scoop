use super::*;
use crate::{
    CheckedNominalSupportCallableSourceV1, CheckedProtectedCallableSourceV1,
    DeclarationAccessSourceV1, ExportDefinitionSourceV1, NominalSourceCallablePayloadV1,
    SourceParameterShapeV1,
};
use scoop_identity::{PersistentGenericTypeId, SignatureTypeKey};
use std::fmt;

/// Definition-side source parameter facts. Implementations resolve the actual
/// declaration and parameter position, not an entry in the transported table.
pub trait ProtectedSourceProtocolSemanticAuthority<E> {
    fn canonical_array_type(&mut self) -> Result<PersistentGenericTypeId, E>;
    fn source_parameter_shape(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<&SourceParameterShapeV1, E>;
    fn source_parameter_calling_kind(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
    ) -> Result<ProtectedParameterCallingKindV1, E>;
    fn validate_source_parameter_origin(
        &self,
        owner: CallableTemplateOrigin,
        position: u32,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), E>;
}

/// The source omission protocol is checked against its declaration. Template
/// bodies, binder mappings and access coverage are closed by the default table.
#[derive(Clone, Copy, Debug)]
pub struct CheckedProtectedSourceProtocolV1<'a> {
    record: &'a ProtectedCallableSourceInterfaceV1,
}
impl<'a> CheckedProtectedSourceProtocolV1<'a> {
    pub const fn record(&self) -> &'a ProtectedCallableSourceInterfaceV1 {
        self.record
    }
}
impl ProtectedCallableSourceInterfaceV1 {
    pub fn validate_protected<'a, A: ProtectedSourceProtocolSemanticAuthority<E>, E>(
        &'a self,
        callable: CheckedProtectedCallableSourceV1<'_>,
        authority: &mut A,
    ) -> Result<CheckedProtectedSourceProtocolV1<'a>, ProtectedSourceSemanticError<E>> {
        self.validate(
            callable.declaration(),
            callable.payload(),
            callable.declaration_access().source(),
            authority,
        )
    }
    pub fn validate_nominal_support<'a, A: ProtectedSourceProtocolSemanticAuthority<E>, E>(
        &'a self,
        callable: CheckedNominalSupportCallableSourceV1<'_>,
        authority: &mut A,
    ) -> Result<CheckedProtectedSourceProtocolV1<'a>, ProtectedSourceSemanticError<E>> {
        self.validate(
            callable.declaration(),
            callable.payload(),
            callable.declaration_access().source(),
            authority,
        )
    }
    pub(in crate::cross_cone_type_semantics) fn validate<
        'a,
        A: ProtectedSourceProtocolSemanticAuthority<E>,
        E,
    >(
        &'a self,
        declaration: CallableTemplateOrigin,
        payload: &NominalSourceCallablePayloadV1,
        access: &DeclarationAccessSourceV1,
        authority: &mut A,
    ) -> Result<CheckedProtectedSourceProtocolV1<'a>, ProtectedSourceSemanticError<E>> {
        use ProtectedSourceSemanticError as Error;
        if self.owner != declaration {
            return Err(Error::Owner);
        }
        let expected = payload.parameters().parameters();
        if expected.len() != self.parameters.parameters().len() {
            return Err(Error::Arity);
        }
        for (position, (actual, expected)) in
            (0_u32..).zip(self.parameters.parameters().iter().zip(expected))
        {
            let source = authority
                .source_parameter_shape(self.owner, position)
                .map_err(Error::Foundation)?;

            if actual.name() != expected.name()
                || actual.name() != source.name()
                || actual.value_type() != expected.value_type()
                || actual.value_type() != source.value_type()
            {
                return Err(Error::Shape { position });
            }
            if actual.calling().kind()
                != authority
                    .source_parameter_calling_kind(self.owner, position)
                    .map_err(Error::Foundation)?
            {
                return Err(Error::Calling { position });
            }
            if let Some(element) = actual.calling().element_type() {
                let array = authority
                    .canonical_array_type()
                    .map_err(Error::Foundation)?;
                let matches = matches!(actual.value_type(), SignatureTypeKey::NominalApplication { origin, arguments } if *origin == array && arguments.as_slice() == std::slice::from_ref(element));
                if !matches {
                    return Err(Error::Vararg { position });
                }
            }
            if actual.definition_origin().origin().source().cone()
                != access.definition_origin().origin().source().cone()
            {
                return Err(Error::Origin { position });
            }
            authority
                .validate_source_parameter_origin(self.owner, position, actual.definition_origin())
                .map_err(Error::Foundation)?;
        }
        Ok(CheckedProtectedSourceProtocolV1 { record: self })
    }
}
#[derive(Debug)]
pub enum ProtectedSourceSemanticError<E> {
    Resource(scoop_wire::WireError),
    Foundation(E),
    Encoding(scoop_wire::cbor::EncodeError),
    Owner,
    Arity,
    Shape { position: u32 },
    Calling { position: u32 },
    Vararg { position: u32 },
    Origin { position: u32 },
}
impl<E: fmt::Display> fmt::Display for ProtectedSourceSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Foundation(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Owner => {
                f.write_str("protected source protocol owner differs from its checked declaration")
            }
            Self::Arity => f.write_str("protected source protocol parameter arity differs"),
            Self::Shape { position } => {
                write!(f, "protected source parameter {position} name/type differs")
            }
            Self::Calling { position } => write!(
                f,
                "protected source parameter {position} omission category differs"
            ),
            Self::Vararg { position } => write!(
                f,
                "protected source parameter {position} is not the canonical Array of its element"
            ),
            Self::Origin { position } => write!(
                f,
                "protected source parameter {position} origin belongs to a different provider"
            ),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedSourceSemanticError<E> {}
