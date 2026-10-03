//! Portable constructor execution, separate from nominal/source declarations.

use std::fmt;

use scoop_identity::{LocalValueSelector, PersistentGenericTypeId, SignatureTypeKey};

use crate::{
    CallableSourceEffectsV1, CanonicalTemplateLocalTableV1, DefaultConstructorRefV1,
    DefaultFieldRefV1, ExportDefinitionSourceV1, ExportTemplateFragmentV1,
    GenericTemplatePredicatesV1,
};

mod sources;
mod wire;
pub use wire::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportConstructorDelegationV1 {
    pub target: DefaultConstructorRefV1,
    pub arguments: ExportTemplateFragmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportPrimaryFieldStoreV1 {
    pub field: DefaultFieldRefV1,
    pub parameter: LocalValueSelector,
}

/// Both stored and delegated properties write their declared backing field.
/// Their distinct source representations remain in the nominal/property table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportCommonInitializationStepV1 {
    Field {
        field: DefaultFieldRefV1,
        value: ExportTemplateFragmentV1,
    },
    Body(ExportTemplateFragmentV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExportConstructorInitializationKindV1 {
    StructPrimary,
    StructSecondary {
        delegation: ExportConstructorDelegationV1,
        body: ExportTemplateFragmentV1,
    },
    ClassPrimary {
        base: Option<ExportConstructorDelegationV1>,
        primary_stores: Vec<ExportPrimaryFieldStoreV1>,
    },
    ClassSecondaryThis {
        delegation: ExportConstructorDelegationV1,
        body: ExportTemplateFragmentV1,
    },
    ClassSecondaryTerminal {
        base: Option<ExportConstructorDelegationV1>,
        body: ExportTemplateFragmentV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportConstructorInitializationV1 {
    declaration: DefaultConstructorRefV1,
    inputs: CanonicalTemplateLocalTableV1,
    effects: CallableSourceEffectsV1,
    predicates: GenericTemplatePredicatesV1,
    definition_origin: ExportDefinitionSourceV1,
    kind: ExportConstructorInitializationKindV1,
}

impl ExportConstructorInitializationV1 {
    pub fn try_new(
        declaration: DefaultConstructorRefV1,
        inputs: CanonicalTemplateLocalTableV1,
        effects: CallableSourceEffectsV1,
        predicates: GenericTemplatePredicatesV1,
        definition_origin: ExportDefinitionSourceV1,
        kind: ExportConstructorInitializationKindV1,
    ) -> Result<Self, GenericInitializationBuildError> {
        use ExportConstructorInitializationKindV1 as Kind;
        let struct_kind = matches!(kind, Kind::StructPrimary | Kind::StructSecondary { .. });
        match (&declaration, struct_kind) {
            (DefaultConstructorRefV1::Struct { .. }, true)
            | (DefaultConstructorRefV1::Class { .. }, false) => {}
            _ => return Err(GenericInitializationBuildError::ConstructorKind),
        }
        if effects.implementation() != crate::CallableImplementationV1::Scoop {
            return Err(GenericInitializationBuildError::BodylessImplementation);
        }
        if let Kind::ClassPrimary { primary_stores, .. } = &kind {
            for store in primary_stores {
                if !matches!(store.field, DefaultFieldRefV1::Class { .. })
                    || !matches!(store.parameter, LocalValueSelector::Parameter { .. })
                    || inputs.get(&store.parameter).is_none()
                {
                    return Err(GenericInitializationBuildError::PrimaryStore);
                }
            }
        }
        match &kind {
            Kind::StructPrimary | Kind::ClassPrimary { .. } => {}
            Kind::StructSecondary { body, .. }
            | Kind::ClassSecondaryThis { body, .. }
            | Kind::ClassSecondaryTerminal { body, .. } => require_body(body)?,
        }
        Ok(Self {
            declaration,
            inputs,
            effects,
            predicates,
            definition_origin,
            kind,
        })
    }

    pub const fn declaration(&self) -> &DefaultConstructorRefV1 {
        &self.declaration
    }

    pub const fn inputs(&self) -> &CanonicalTemplateLocalTableV1 {
        &self.inputs
    }

    pub fn effects(&self) -> CallableSourceEffectsV1 {
        self.effects.clone()
    }

    pub const fn predicates(&self) -> &GenericTemplatePredicatesV1 {
        &self.predicates
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }

    pub const fn kind(&self) -> &ExportConstructorInitializationKindV1 {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportGenericNominalInitializationV1 {
    owner: PersistentGenericTypeId,
    common: Vec<ExportCommonInitializationStepV1>,
    constructors: Vec<ExportConstructorInitializationV1>,
}

impl ExportGenericNominalInitializationV1 {
    pub fn try_new(
        owner: PersistentGenericTypeId,
        common: Vec<ExportCommonInitializationStepV1>,
        mut constructors: Vec<ExportConstructorInitializationV1>,
    ) -> Result<Self, GenericInitializationBuildError> {
        constructors.sort_unstable_by(|left, right| left.declaration.cmp(&right.declaration));
        Self::from_canonical(owner, common, constructors)
    }

    fn from_canonical(
        owner: PersistentGenericTypeId,
        common: Vec<ExportCommonInitializationStepV1>,
        constructors: Vec<ExportConstructorInitializationV1>,
    ) -> Result<Self, GenericInitializationBuildError> {
        if constructors.is_empty() {
            return Err(GenericInitializationBuildError::MissingConstructors);
        }
        validate_constructors(owner, &constructors)?;
        for step in &common {
            match step {
                ExportCommonInitializationStepV1::Field { field, value } => {
                    if !matches!(field, DefaultFieldRefV1::Class { .. })
                        || value.results().len() != 1
                    {
                        return Err(GenericInitializationBuildError::FieldInitializer);
                    }
                }
                ExportCommonInitializationStepV1::Body(body) => require_body(body)?,
            }
        }
        if !common.is_empty()
            && constructors.iter().any(|constructor| {
                matches!(
                    constructor.declaration,
                    DefaultConstructorRefV1::Struct { .. }
                )
            })
        {
            return Err(GenericInitializationBuildError::StructCommonInitialization);
        }
        Ok(Self {
            owner,
            common,
            constructors,
        })
    }

    pub const fn owner(&self) -> PersistentGenericTypeId {
        self.owner
    }

    pub fn common(&self) -> &[ExportCommonInitializationStepV1] {
        &self.common
    }

    pub fn constructors(&self) -> &[ExportConstructorInitializationV1] {
        &self.constructors
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalExportGenericInitializationsV1 {
    records: Vec<ExportGenericNominalInitializationV1>,
}

impl CanonicalExportGenericInitializationsV1 {
    pub fn try_new(
        mut records: Vec<ExportGenericNominalInitializationV1>,
    ) -> Result<Self, GenericInitializationBuildError> {
        records.sort_unstable_by_key(ExportGenericNominalInitializationV1::owner);
        require_owner_order(&records)?;
        Ok(Self { records })
    }

    pub fn records(&self) -> &[ExportGenericNominalInitializationV1] {
        &self.records
    }

    pub fn get(
        &self,
        owner: PersistentGenericTypeId,
    ) -> Option<&ExportGenericNominalInitializationV1> {
        self.records
            .binary_search_by_key(&owner, ExportGenericNominalInitializationV1::owner)
            .ok()
            .map(|index| &self.records[index])
    }
}

fn require_body(body: &ExportTemplateFragmentV1) -> Result<(), GenericInitializationBuildError> {
    if body.results().is_empty() {
        Ok(())
    } else {
        Err(GenericInitializationBuildError::StatementResults)
    }
}

fn validate_constructors(
    owner: PersistentGenericTypeId,
    constructors: &[ExportConstructorInitializationV1],
) -> Result<(), GenericInitializationBuildError> {
    for constructor in constructors {
        if !matches!(constructor.declaration.owner_type(), SignatureTypeKey::NominalApplication { origin, .. } if *origin == owner)
        {
            return Err(GenericInitializationBuildError::ConstructorOwner);
        }
    }
    for pair in constructors.windows(2) {
        if pair[0].declaration >= pair[1].declaration {
            return Err(GenericInitializationBuildError::ConstructorOrder);
        }
    }
    Ok(())
}

fn require_owner_order(
    records: &[ExportGenericNominalInitializationV1],
) -> Result<(), GenericInitializationBuildError> {
    if records
        .windows(2)
        .any(|pair| pair[0].owner >= pair[1].owner)
    {
        Err(GenericInitializationBuildError::NominalOrder)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenericInitializationBuildError {
    MissingConstructors,
    ConstructorKind,
    ConstructorOwner,
    ConstructorOrder,
    NominalOrder,
    BodylessImplementation,
    PrimaryStore,
    FieldInitializer,
    StatementResults,
    StructCommonInitialization,
}

impl fmt::Display for GenericInitializationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingConstructors => "nominal initialization template has no constructors",
            Self::ConstructorKind => "constructor template kind differs from its declaration",
            Self::ConstructorOwner => "constructor template has a different nominal owner",
            Self::ConstructorOrder => "constructor templates are duplicated or out of order",
            Self::NominalOrder => "nominal initialization templates are duplicated or out of order",
            Self::BodylessImplementation => "bodyless constructor has an execution template",
            Self::PrimaryStore => {
                "primary field store does not reference a class field and parameter"
            }
            Self::FieldInitializer => "class field initializer must yield exactly one value",
            Self::StatementResults => "statement-only initialization fragment yields a value",
            Self::StructCommonInitialization => "struct template has class common initialization",
        })
    }
}
impl std::error::Error for GenericInitializationBuildError {}
