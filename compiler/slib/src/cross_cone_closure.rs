//! Graph and provider-role validation for the M23-5 semantic closure.

use std::collections::BTreeMap;

use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::ValidatedLirTargetSelection;

use crate::{
    DecodedCrossConeHirFrontSections, FoundationValidatedCrossConeHirFrontSections,
    HirProductionValidatedCrossConeHirFrontSections, ResolvedCrossConeHirFrontSections,
};

mod errors;
pub use errors::*;

pub(crate) mod graph_validation;
mod hir_front_validation;
mod identity_validation;
mod source_provenance;
pub use source_provenance::CrossConeSourceProvenanceError;

mod surface_validation;
pub use surface_validation::*;

mod route_validation;
pub use route_validation::*;

mod external_reference_validation;
pub use external_reference_validation::*;

mod type_alias_expansion;
pub use type_alias_expansion::*;

mod mir_bridge_validation;
pub use mir_bridge_validation::*;

mod lir_bridge_validation;
pub use lir_bridge_validation::*;

mod commit;
pub use commit::*;

mod validate;
pub use validate::*;

mod artifact;
pub use artifact::*;

/// Untrusted assembly input for the dependency artifacts visible while
/// compiling one current Cone.
pub struct DecodedCrossConeClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
    current_artifact: Option<DecodedCrossConeHirFrontSections<'input>>,
}

/// Directness is a closed role assigned only after the whole dependency
/// graph has been checked. A support provider cannot be promoted by a bool.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CrossConeProviderRole {
    Direct,
    Support,
}

/// Cross-Cone HIR fronts whose exact profile and complete dependency graph
/// agree. Identity, surface, route, and bridge obligations remain unproven.
pub struct ProfileValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<DecodedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Cross-Cone HIR fronts whose foundation identities were rehashed and
/// resolved against their exact dependency authority. No graph has been
/// committed to the caller's semantic session yet.
pub struct IdentityRegisteredCrossConeHirClosure<'input> {
    profile: ProfileValidatedCrossConeHirClosure<'input>,
    identity_graphs: Vec<ValidatedIdentityGraph>,
}

/// Cross-Cone HIR fronts with fully checked ODR-free foundations. General HIR
/// surfaces and legacy production payloads remain unresolved and unvalidated.
pub struct FoundationValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<FoundationValidatedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Cross-Cone providers whose general HIR wire references are typed and
/// resolved. Semantic ownership, public-surface, and route closure are still
/// pending, so this state cannot be imported into a world or session.
pub struct ResolvedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<ResolvedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

/// Cross-Cone providers whose foundations, general HIR references, and legacy
/// direct-public surfaces are validated. General surface semantics and routes
/// remain pending, so this state still cannot be imported.
pub struct HirProductionValidatedCrossConeHirClosure<'input> {
    current: ConeIdentity,
    target: ValidatedLirTargetSelection,
    direct: Vec<ConeIdentity>,
    dependency_first: Vec<HirProductionValidatedCrossConeHirFrontSections<'input>>,
    positions: BTreeMap<ConeIdentity, usize>,
    dependency_positions: Vec<Vec<usize>>,
}

#[cfg(test)]
mod tests;
