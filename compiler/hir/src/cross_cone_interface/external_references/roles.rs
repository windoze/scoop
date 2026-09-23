use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

/// Closed reason set for retaining one foreign typed reference in the HIR
/// interface closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExternalHirReferenceRoleV1 {
    ReexportTarget,
    SignatureDependency,
    AliasTarget,
    DefaultDependency,
    ConstType,
    ConcreteSelectedUse,
}

impl ExternalHirReferenceRoleV1 {
    /// Fixed language types have no source-name lookup or export binding.
    /// Their typed provider and actual dependency use remain required.
    pub(crate) fn requires_source_name_witness(self, target: super::ExternalHirTargetV1) -> bool {
        match self {
            Self::ReexportTarget | Self::AliasTarget | Self::ConcreteSelectedUse => true,
            Self::SignatureDependency | Self::ConstType => false,
            Self::DefaultDependency => {
                use scoop_identity::{CoreBuiltinNominal, NominalDeclarationOwner};
                let super::ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(id)) =
                    target
                else {
                    return true;
                };
                static BUILTINS: std::sync::LazyLock<[scoop_identity::PersistentTypeId; 2]> =
                    std::sync::LazyLock::new(|| {
                        [
                            CoreBuiltinNominal::Unit.identity_record().id(),
                            CoreBuiltinNominal::Any.identity_record().id(),
                        ]
                    });
                !BUILTINS.contains(&id)
            }
        }
    }
}

impl WireEncode for ExternalHirReferenceRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::ReexportTarget => 1,
            Self::SignatureDependency => 2,
            Self::AliasTarget => 3,
            Self::DefaultDependency => 4,
            Self::ConstType => 5,
            Self::ConcreteSelectedUse => 6,
        })
    }
}

impl WireDecode for ExternalHirReferenceRoleV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ReexportTarget),
            2 => Ok(Self::SignatureDependency),
            3 => Ok(Self::AliasTarget),
            4 => Ok(Self::DefaultDependency),
            5 => Ok(Self::ConstType),
            6 => Ok(Self::ConcreteSelectedUse),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

/// Non-empty canonical set of reasons attached to one external reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExternalHirReferenceRolesV1 {
    roles: Vec<ExternalHirReferenceRoleV1>,
}

impl CanonicalExternalHirReferenceRolesV1 {
    pub fn try_new(
        mut roles: Vec<ExternalHirReferenceRoleV1>,
    ) -> Result<Self, ExternalHirReferenceRoleSetBuildError> {
        if roles.is_empty() {
            return Err(ExternalHirReferenceRoleSetBuildError::Empty);
        }
        roles.sort_unstable();
        if let Some(role) = roles
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
        {
            return Err(ExternalHirReferenceRoleSetBuildError::Duplicate(role));
        }
        Ok(Self { roles })
    }

    pub fn roles(&self) -> &[ExternalHirReferenceRoleV1] {
        &self.roles
    }

    pub fn contains(&self, role: ExternalHirReferenceRoleV1) -> bool {
        self.roles.binary_search(&role).is_ok()
    }
}

impl WireEncode for CanonicalExternalHirReferenceRolesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.roles.len() as u64)?;
        for role in &self.roles {
            role.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExternalHirReferenceRolesV1 {
    roles: Vec<ExternalHirReferenceRoleV1>,
}

impl DecodedCanonicalExternalHirReferenceRolesV1 {
    pub fn validate(
        self,
    ) -> Result<CanonicalExternalHirReferenceRolesV1, ExternalHirReferenceRoleSetValidationError>
    {
        if self.roles.is_empty() {
            return Err(ExternalHirReferenceRoleSetValidationError::Empty);
        }
        for (index, pair) in self.roles.windows(2).enumerate() {
            let index = index + 1;
            match pair[0].cmp(&pair[1]) {
                std::cmp::Ordering::Equal => {
                    return Err(ExternalHirReferenceRoleSetValidationError::Duplicate {
                        index,
                        role: pair[1],
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(
                        ExternalHirReferenceRoleSetValidationError::NonCanonicalOrder { index },
                    );
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(CanonicalExternalHirReferenceRolesV1 { roles: self.roles })
    }
}

impl WireEncode for DecodedCanonicalExternalHirReferenceRolesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.roles.len() as u64)?;
        for role in &self.roles {
            role.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalExternalHirReferenceRolesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| ExternalHirReferenceRoleV1::decode(decoder))
            .map(|roles| Self { roles })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHirReferenceRoleSetBuildError {
    Empty,
    Duplicate(ExternalHirReferenceRoleV1),
}

impl fmt::Display for ExternalHirReferenceRoleSetBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("external HIR reference role set must not be empty"),
            Self::Duplicate(role) => {
                write!(formatter, "duplicate external HIR reference role {role:?}")
            }
        }
    }
}

impl std::error::Error for ExternalHirReferenceRoleSetBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHirReferenceRoleSetValidationError {
    Empty,
    Duplicate {
        index: usize,
        role: ExternalHirReferenceRoleV1,
    },
    NonCanonicalOrder {
        index: usize,
    },
}

impl fmt::Display for ExternalHirReferenceRoleSetValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("external HIR reference role set must not be empty"),
            Self::Duplicate { index, role } => write!(
                formatter,
                "duplicate external HIR reference role {role:?} at index {index}"
            ),
            Self::NonCanonicalOrder { index } => write!(
                formatter,
                "non-canonical external HIR reference role order at index {index}"
            ),
        }
    }
}

impl std::error::Error for ExternalHirReferenceRoleSetValidationError {}

#[cfg(test)]
mod tests;
