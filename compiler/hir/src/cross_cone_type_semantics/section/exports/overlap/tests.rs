//! Narrow joins use real typed constituents; no checked section is fabricated.
use super::*;
use crate::cross_cone_interface::expression_test_support::Fixture as ExpressionFixture;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::Fixture as SourceFixture;
use scoop_identity::{
    CanonicalIdentifier, CoreBuiltinNominal, LocalValueSelector, PersistentFunctionId,
    SourceDeclarationKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

mod accessors;
mod calls;
mod constants;
mod defaults;
mod protocols;
mod support;
use support::*;
