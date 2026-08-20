//! Complete source initialization templates for delegated extension properties.

use scoop_identity::{PersistentExtensionPropertyId, SignatureTypeKey};
use scoop_wire::{WireError, WirePath};

use crate::{DefaultCallableDeclarationV1, ExportGenericCallableBodyV1};

mod table;
mod wire;
pub use table::*;
pub use wire::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportGenericDelegateTemplateV1 {
    property: PersistentExtensionPropertyId,
    effective_type: SignatureTypeKey,
    initializer: ExportGenericCallableBodyV1,
    diagnostic_path: String,
}

impl ExportGenericDelegateTemplateV1 {
    pub fn try_new(
        property: PersistentExtensionPropertyId,
        effective_type: SignatureTypeKey,
        initializer: ExportGenericCallableBodyV1,
        diagnostic_path: String,
    ) -> Result<Self, GenericDelegateTemplateBuildError> {
        let unit = scoop_identity::PersistentInitializationUnitId::from_key(
            &scoop_identity::InitializationUnitKey::ExtensionProperty(property),
        )
        .map_err(|_| GenericDelegateTemplateBuildError::InitializerShape(property))?;
        let owner = scoop_identity::PersistentGeneratedCallableId::from_key(
            &scoop_identity::GeneratedCallableKey::Initialization {
                unit,
                role: scoop_identity::InitializationCallableRole::Initializer,
            },
        )
        .map_err(|_| GenericDelegateTemplateBuildError::InitializerShape(property))?;
        if initializer.owner() != DefaultCallableDeclarationV1::Generated(owner)
            || initializer.type_parameters().arguments().is_empty()
            || !initializer.parameters().is_empty()
            || !initializer.capture_types().is_empty()
        {
            return Err(GenericDelegateTemplateBuildError::InitializerShape(
                property,
            ));
        }
        if diagnostic_path.is_empty() || diagnostic_path.contains('\0') {
            return Err(GenericDelegateTemplateBuildError::DiagnosticPath(property));
        }
        Ok(Self {
            property,
            effective_type,
            initializer,
            diagnostic_path,
        })
    }

    pub const fn property(&self) -> PersistentExtensionPropertyId {
        self.property
    }

    pub const fn effective_type(&self) -> &SignatureTypeKey {
        &self.effective_type
    }

    pub const fn initializer(&self) -> &ExportGenericCallableBodyV1 {
        &self.initializer
    }

    pub fn diagnostic_path(&self) -> &str {
        &self.diagnostic_path
    }

    pub fn visit_direct_references<'body, V: crate::DefaultBodyReferenceVisitorV1<'body>>(
        &'body self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), V::Error> {
        visitor.reference(
            crate::DefaultBodyReferenceOccurrenceV1 {
                target: crate::DefaultBodyReferenceTargetV1::Type(&self.effective_type),
                definition_origin: self.initializer.definition_origin(),
                site: crate::ExportDefaultReferenceOccurrenceSiteV1::InitializationHeader,
                attachment: crate::DefaultBodyReferenceAttachmentV1::Metadata(
                    crate::DefaultBodyReferenceMetadataV1::CallableBody(&self.initializer),
                ),
            },
            path,
        )?;
        self.initializer.visit_direct_references(visitor, path)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenericDelegateTemplateBuildError {
    InitializerShape(PersistentExtensionPropertyId),
    DiagnosticPath(PersistentExtensionPropertyId),
    RecordOrder {
        index: usize,
        property: PersistentExtensionPropertyId,
    },
}

impl std::fmt::Display for GenericDelegateTemplateBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid generic delegate template: {self:?}")
    }
}
impl std::error::Error for GenericDelegateTemplateBuildError {}

#[derive(Debug)]
pub enum GenericDelegateTemplateResolutionError<E> {
    Identity(E),
    Body(Box<crate::GenericCallableBodyResolutionError<E>>),
    Record(GenericDelegateTemplateBuildError),
}

impl<E: std::fmt::Display> std::fmt::Display for GenericDelegateTemplateResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => write!(f, "invalid generic delegate reference: {error}"),
            Self::Body(error) => write!(f, "invalid generic delegate initializer: {error}"),
            Self::Record(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for GenericDelegateTemplateResolutionError<E>
{
}
