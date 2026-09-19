use super::*;
use crate::cross_cone_type_semantics::inheritance::inheritance_interface_fixture;
use crate::cross_cone_type_semantics::inheritance::tests::support::site;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::{
    Fixture as SourceFixture, nominal,
};
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, Effect, GeneratedCallableKey,
    LexicalCallableParent, LexicalCallableRole, LocalValueSelector, NonEmptyVec, PersistentFieldId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericTypeId, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceNominalKind, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

mod authority;
mod cases;
mod fixture;
mod protocol;
mod run;
mod template;
use authority::Authority;
use fixture::Fixture;
use run::{run, run_with_limits};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Valid,
    Generic,
    GenericReject,
    Flow,
    BeforeDefinition,
    Nested,
    NestedAbi,
    Contract,
    BodyEnvelope,
    Origin,
    OperationType,
    MissingMetadata,
    WrongUse,
    ReferenceDomain,
    ReferenceSource,
    MissingDefault,
    ExtraDefault,
}
impl Case {
    fn generic(self) -> bool {
        matches!(self, Self::Generic | Self::GenericReject)
    }
    fn flow(self) -> bool {
        matches!(self, Self::Flow | Self::BeforeDefinition)
    }
    fn nested(self) -> bool {
        matches!(self, Self::Nested | Self::NestedAbi)
    }
}
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn parameter() -> LocalValueSelector {
    LocalValueSelector::Parameter {
        declaration_index: 0,
    }
}
fn definition_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 1),
        [],
    )
}
fn nested_path(role: StructuralDefinitionSiteRole) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 1),
        [StructuralPathSegment::new(role, 0)],
    )
}
fn declared_local() -> LocalValueSelector {
    LocalValueSelector::LocalDeclaration {
        path: nested_path(StructuralDefinitionSiteRole::LocalDeclaration),
    }
}
