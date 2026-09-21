use scoop_wire::HashError;
use std::fmt;

use scoop_identity::{
    ConeIdentity, LinkageClass, ObjectDefinitionIdentityError, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentExactTypeId, PersistentSymbolError, PersistentSymbolKey,
    PersistentSymbolRequest, StrongDefinitionEntity, StrongDefinitionRole,
};

/// A descriptor imported from its actual provider. Foundation and layout/ABI
/// selections retain the same complete physical definition contract.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExternalTypeDescriptor {
    provider: ConeIdentity,
    target: PersistentExactTypeId,
    expected_symbol: PersistentSymbolRequest,
    required_definition: ObjectDefinitionPlanId,
}

impl ExternalTypeDescriptor {
    pub fn new(
        provider: ConeIdentity,
        target: PersistentExactTypeId,
    ) -> Result<Self, ExternalTypeDescriptorBuildError> {
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::TypeDescriptor(target),
            LinkageClass::ConeStrong,
        )
        .map_err(ExternalTypeDescriptorBuildError::Symbol)?;
        let key = ObjectDefinitionPlanKey::strong(
            provider,
            StrongDefinitionEntity::exact_type(target),
            StrongDefinitionRole::TypeDescriptor,
        )
        .map_err(ExternalTypeDescriptorBuildError::Definition)?;
        let definition = ObjectDefinitionPlanId::from_key(&key)
            .map_err(ExternalTypeDescriptorBuildError::Identity)?;
        Ok(Self::from_selection(provider, target, symbol, definition))
    }

    pub(crate) const fn from_selection(
        provider: ConeIdentity,
        target: PersistentExactTypeId,
        expected_symbol: PersistentSymbolRequest,
        required_definition: ObjectDefinitionPlanId,
    ) -> Self {
        Self {
            provider,
            target,
            expected_symbol,
            required_definition,
        }
    }

    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub const fn target(self) -> PersistentExactTypeId {
        self.target
    }

    pub const fn expected_symbol(self) -> PersistentSymbolRequest {
        self.expected_symbol
    }

    pub const fn required_definition(self) -> ObjectDefinitionPlanId {
        self.required_definition
    }
}

#[derive(Debug)]
pub enum ExternalTypeDescriptorBuildError {
    Identity(HashError),
    Symbol(PersistentSymbolError),
    Definition(ObjectDefinitionIdentityError),
}

impl fmt::Display for ExternalTypeDescriptorBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot derive external TypeDescriptor: {self:?}")
    }
}

impl std::error::Error for ExternalTypeDescriptorBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
            Self::Symbol(error) => Some(error),
            Self::Definition(error) => Some(error),
        }
    }
}
