use super::{ProtectedDefaultTemplateKeyV1, ProtectedSourceBuildError};
use crate::ExportDefinitionSourceV1;
use scoop_identity::{CanonicalIdentifier, SignatureTypeKey};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedParameterCallingKindV1 {
    Required,
    Default,
    VarargEmpty,
    VarargDefault,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtectedParameterCallingV1 {
    Required,
    Default {
        template: ProtectedDefaultTemplateKeyV1,
    },
    VarargEmpty {
        element_type: SignatureTypeKey,
    },
    VarargDefault {
        element_type: SignatureTypeKey,
        template: ProtectedDefaultTemplateKeyV1,
    },
}
impl ProtectedParameterCallingV1 {
    pub const fn kind(&self) -> ProtectedParameterCallingKindV1 {
        match self {
            Self::Required => ProtectedParameterCallingKindV1::Required,
            Self::Default { .. } => ProtectedParameterCallingKindV1::Default,
            Self::VarargEmpty { .. } => ProtectedParameterCallingKindV1::VarargEmpty,
            Self::VarargDefault { .. } => ProtectedParameterCallingKindV1::VarargDefault,
        }
    }
    pub const fn template(&self) -> Option<ProtectedDefaultTemplateKeyV1> {
        match self {
            Self::Default { template } | Self::VarargDefault { template, .. } => Some(*template),
            Self::Required | Self::VarargEmpty { .. } => None,
        }
    }
    pub const fn element_type(&self) -> Option<&SignatureTypeKey> {
        match self {
            Self::VarargEmpty { element_type } | Self::VarargDefault { element_type, .. } => {
                Some(element_type)
            }
            Self::Required | Self::Default { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedSourceParameterV1 {
    name: CanonicalIdentifier,
    value_type: SignatureTypeKey,
    calling: ProtectedParameterCallingV1,
    definition_origin: ExportDefinitionSourceV1,
}
impl ProtectedSourceParameterV1 {
    pub const fn new(
        name: CanonicalIdentifier,
        value_type: SignatureTypeKey,
        calling: ProtectedParameterCallingV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Self {
        Self {
            name,
            value_type,
            calling,
            definition_origin,
        }
    }
    pub const fn name(&self) -> &CanonicalIdentifier {
        &self.name
    }
    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
    pub const fn calling(&self) -> &ProtectedParameterCallingV1 {
        &self.calling
    }
    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalProtectedSourceParametersV1 {
    parameters: Vec<ProtectedSourceParameterV1>,
    len: u32,
}
impl CanonicalProtectedSourceParametersV1 {
    pub fn try_new(
        parameters: Vec<ProtectedSourceParameterV1>,
    ) -> Result<Self, ProtectedSourceBuildError> {
        let len =
            u32::try_from(parameters.len()).map_err(|_| ProtectedSourceBuildError::TooMany)?;
        let mut names = BTreeSet::new();
        let mut vararg = false;
        for (position, parameter) in (0_u32..).zip(&parameters) {
            if !names.insert(parameter.name()) {
                return Err(ProtectedSourceBuildError::DuplicateName { position });
            }
            if parameter.calling().element_type().is_some() && std::mem::replace(&mut vararg, true)
            {
                return Err(ProtectedSourceBuildError::MultipleVarargs);
            }
        }
        Ok(Self { parameters, len })
    }
    pub fn parameters(&self) -> &[ProtectedSourceParameterV1] {
        &self.parameters
    }
    pub const fn len_u32(&self) -> u32 {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}
