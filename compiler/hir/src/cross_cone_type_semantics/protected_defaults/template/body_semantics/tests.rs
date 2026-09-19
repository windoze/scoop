use super::*;
use crate::cross_cone_interface::expression_test_support::Fixture;
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, Effect, GeneratedCallableKey, LexicalCallableParent,
    LexicalCallableRole, LocalValueSelector, PersistentFieldId, PersistentGeneratedCallableId,
    SignatureTypeKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment, SyntheticLocalRole,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath};

mod flow;
mod nested;
mod operations;
mod support;
use support::*;
