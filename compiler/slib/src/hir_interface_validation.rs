//! Shared declaration checks over the actual decoded HIR metadata.

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1, OdrFreeHirFoundation,
};
use scoop_identity::{ConeIdentity, ValidatedIdentityGraph};
use scoop_wire::BudgetMeter;

use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, ValidatedNominalProviderView,
};

mod call_sites;
mod declarations;
mod references;
mod sources;
pub use call_sites::CrossConeHirCallSiteOriginError;
pub use references::CrossConeHirReferenceSurfaceError;

#[derive(Clone, Copy)]
pub(crate) struct HirInterfaceValidationInput<'a> {
    pub(crate) current: ConeIdentity,
    pub(crate) identities: &'a ValidatedIdentityGraph,
    pub(crate) foundation: &'a OdrFreeHirFoundation,
    pub(crate) core: &'a CoreBootstrapInterfaceSectionV1,
    pub(crate) interface: &'a CrossConeHirInterfaceSectionV1,
}
