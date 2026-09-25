use super::*;
use scoop_identity::{
    DefinitionAtomRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanId, PersistentSymbolError, PersistentSymbolRequest,
};
use scoop_wire::WireError;

/// A complete physical definition relation. This does not authorize an import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongShapeDefinitionRefV1 {
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
    symbol: PersistentSymbolRequest,
    definition: ObjectDefinitionPlanId,
    primary: ObjectDefinitionAtomId,
}

impl StrongShapeDefinitionRefV1 {
    pub fn from_foundation(
        subject: ExternalStrongShapeSubjectV1,
        foundation: &crate::OdrFreeLirFoundation,
    ) -> Result<Self, StrongShapeDefinitionError> {
        let (key, symbol) = subject.expected_definition(foundation.producer())?;

        let definition = foundation
            .definition_plans()
            .iter()
            .find(|record| record.key() == &key)
            .ok_or(StrongShapeDefinitionError::MissingDefinition(key))?;
        let symbol = PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong)
            .map_err(StrongShapeDefinitionError::Symbol)?;

        if !foundation.contains_symbol_request(symbol) {
            return Err(StrongShapeDefinitionError::MissingSymbol(symbol));
        }

        let mut primary = foundation.definition_atoms().iter().filter(|record| {
            record.key().plan() == definition.id()
                && record.key().role() == DefinitionAtomRole::Primary
        });
        let primary = match (primary.next(), primary.next()) {
            (Some(record), None) => record.id(),
            _ => return Err(StrongShapeDefinitionError::PrimaryAtomSet(definition.id())),
        };
        Ok(Self {
            provider: foundation.producer(),
            subject,
            symbol,
            definition: definition.id(),
            primary,
        })
    }
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }
    pub const fn subject(&self) -> ExternalStrongShapeSubjectV1 {
        self.subject
    }
    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }
    pub const fn definition(&self) -> ObjectDefinitionPlanId {
        self.definition
    }
    pub const fn primary(&self) -> ObjectDefinitionAtomId {
        self.primary
    }
}

#[derive(Debug)]
pub enum StrongShapeDefinitionError {
    Hash(HashError),
    DefinitionIdentity(ObjectDefinitionIdentityError),
    Symbol(PersistentSymbolError),
    MissingDefinition(ObjectDefinitionPlanKey),
    MissingSymbol(PersistentSymbolRequest),
    PrimaryAtomSet(ObjectDefinitionPlanId),
    Resource(WireError),
}
impl From<HashError> for StrongShapeDefinitionError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}
impl From<WireError> for StrongShapeDefinitionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for StrongShapeDefinitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid Strong shape definition: {self:?}")
    }
}
impl std::error::Error for StrongShapeDefinitionError {}
