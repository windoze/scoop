//! Per-artifact HIR semantic-surface type-state transitions.

use scoop_hir::{
    CallableInterfaceSetSemanticValidationError, CoreBootstrapInterfaceSectionV1,
    CrossConeHirInterfaceSectionV1, CrossConeHirInternalClosureValidationError,
    NominalInterfaceSetSemanticValidationError, OdrFreeHirFoundation,
    PropertyInterfaceSetSemanticValidationError, TypeAliasInterfaceSetSemanticValidationError,
};
use scoop_identity::{ConeCoordinate, ConeIdentity, ValidatedIdentityGraph};
use scoop_lir::{DecodedStrongProductionSectionV1, OdrFreeLirFoundation};
use scoop_mir::{DecodedCoreBootstrapBridgeSectionV1, OdrFreeMirFoundation};
use scoop_wire::WirePath;

use super::HirProductionValidatedCrossConeHirFrontSections;
use crate::{
    ValidatedGraphArtifact,
    cross_cone_hir_authority::{
        CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError,
        ValidatedNominalProviderView,
    },
    strong_compile_decode::OdrFreeStrongFoundationSet,
};

/// Storage shared by the declaration-surface proof states. Each public
/// wrapper below is a distinct, consuming type-state gate over this carrier.
struct ValidatedSurfaceFront<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_strong_production: DecodedStrongProductionSectionV1,
}

/// One provider whose exact section-internal HIR relationships match the
/// legacy direct surface. Cross-provider declaration authority is pending.
pub struct InternallyClosedCrossConeHirFrontSections<'input>(ValidatedSurfaceFront<'input>);

/// One provider whose public nominal table has canonical typed ownership.
pub struct NominalValidatedCrossConeHirFrontSections<'input>(ValidatedSurfaceFront<'input>);

/// One provider whose public nominal and property tables are canonical.
pub struct PropertyValidatedCrossConeHirFrontSections<'input>(ValidatedSurfaceFront<'input>);

/// One provider whose public nominal, property, and callable tables are
/// canonical.
pub struct CallableValidatedCrossConeHirFrontSections<'input>(ValidatedSurfaceFront<'input>);

/// One provider whose public nominal, property, callable, and type-alias
/// tables have canonical typed authority.
pub struct TypeAliasValidatedCrossConeHirFrontSections<'input>(ValidatedSurfaceFront<'input>);

macro_rules! impl_surface_front_accessors {
    ($state:ident) => {
        impl $state<'_> {
            pub const fn coordinate(&self) -> &ConeCoordinate {
                self.0.graph.coordinate()
            }

            pub const fn identity(&self) -> ConeIdentity {
                self.0.graph.identity()
            }

            pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
                &self.0.foundations.hir
            }

            pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
                &self.0.foundations.mir
            }

            pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
                &self.0.foundations.lir
            }

            pub fn identity_count(&self) -> usize {
                self.0.identities.identity_count()
            }

            pub fn declared_identity_count(&self) -> usize {
                self.0.identities.declared_identity_count()
            }

            pub const fn hir_core_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
                &self.0.hir_core_production
            }

            pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
                &self.0.hir_interface
            }

            pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
                &self.0.mir_core_production
            }

            pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
                &self.0.lir_strong_production
            }
        }
    };
}

impl_surface_front_accessors!(InternallyClosedCrossConeHirFrontSections);
impl_surface_front_accessors!(NominalValidatedCrossConeHirFrontSections);
impl_surface_front_accessors!(PropertyValidatedCrossConeHirFrontSections);
impl_surface_front_accessors!(CallableValidatedCrossConeHirFrontSections);
impl_surface_front_accessors!(TypeAliasValidatedCrossConeHirFrontSections);

macro_rules! impl_nominal_provider_view {
    ($state:ident) => {
        impl $state<'_> {
            pub(crate) const fn nominal_provider_view(&self) -> ValidatedNominalProviderView<'_> {
                ValidatedNominalProviderView {
                    identity: self.0.graph.identity(),
                    interface: &self.0.hir_interface,
                }
            }
        }
    };
}

impl_nominal_provider_view!(NominalValidatedCrossConeHirFrontSections);
impl_nominal_provider_view!(PropertyValidatedCrossConeHirFrontSections);
impl_nominal_provider_view!(CallableValidatedCrossConeHirFrontSections);
impl_nominal_provider_view!(TypeAliasValidatedCrossConeHirFrontSections);

impl<'input> HirProductionValidatedCrossConeHirFrontSections<'input> {
    /// Closes every relationship reconstructible from this HIR section and
    /// its independently validated direct-public surface.
    pub(crate) fn validate_internal_hir_closures(
        self,
    ) -> Result<InternallyClosedCrossConeHirFrontSections<'input>, CrossConeHirInternalClosureError>
    {
        let Self {
            mut graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self;
        hir_interface
            .validate_internal_closures(
                hir_core_production.direct_public_surface(),
                graph.envelope.meter_mut(),
                &WirePath::root(),
            )
            .map_err(CrossConeHirInternalClosureError::Interface)?;
        Ok(InternallyClosedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                lir_strong_production,
            },
        ))
    }
}

impl<'input> InternallyClosedCrossConeHirFrontSections<'input> {
    /// Validates the nominal declaration surface using canonical keys and
    /// only the provider's already validated transitive dependencies.
    pub(crate) fn validate_nominal_surface<'dependency>(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'dependency>>,
    ) -> Result<NominalValidatedCrossConeHirFrontSections<'input>, CrossConeHirNominalSurfaceError>
    {
        let ValidatedSurfaceFront {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_interface,
            dependencies,
        );
        hir_interface
            .nominal_interfaces()
            .validate_semantics(&mut authority)
            .map_err(|error| CrossConeHirNominalSurfaceError::NominalInterfaces(Box::new(error)))?;
        Ok(NominalValidatedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                lir_strong_production,
            },
        ))
    }
}

impl<'input> NominalValidatedCrossConeHirFrontSections<'input> {
    /// Validates each property identity, signature scope, and accessor key.
    pub(crate) fn validate_property_surface<'dependency>(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'dependency>>,
    ) -> Result<PropertyValidatedCrossConeHirFrontSections<'input>, CrossConeHirPropertySurfaceError>
    {
        let ValidatedSurfaceFront {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_interface,
            dependencies,
        );
        hir_interface
            .property_interfaces()
            .validate_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirPropertySurfaceError::PropertyInterfaces(Box::new(error))
            })?;
        Ok(PropertyValidatedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                lir_strong_production,
            },
        ))
    }
}

impl<'input> PropertyValidatedCrossConeHirFrontSections<'input> {
    /// Validates each callable identity shape against its kind-specific key
    /// and the already property-validated adjacent record.
    pub(crate) fn validate_callable_surface<'dependency>(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'dependency>>,
    ) -> Result<CallableValidatedCrossConeHirFrontSections<'input>, CrossConeHirCallableSurfaceError>
    {
        let ValidatedSurfaceFront {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_interface,
            dependencies,
        );
        hir_interface
            .callable_interfaces()
            .validate_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirCallableSurfaceError::CallableInterfaces(Box::new(error))
            })?;
        Ok(CallableValidatedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                lir_strong_production,
            },
        ))
    }
}

impl<'input> CallableValidatedCrossConeHirFrontSections<'input> {
    /// Validates each public non-generic type alias against its declaration,
    /// direct-public proof, foundation origin, and target shape.
    pub(crate) fn validate_type_alias_surface<'dependency>(
        self,
        dependencies: Vec<ValidatedNominalProviderView<'dependency>>,
    ) -> Result<
        TypeAliasValidatedCrossConeHirFrontSections<'input>,
        CrossConeHirTypeAliasSurfaceError,
    > {
        let ValidatedSurfaceFront {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self.0;
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            graph.identity(),
            &identities,
            &foundations.hir,
            &hir_interface,
            dependencies,
        );
        hir_interface
            .type_aliases()
            .validate_semantics(&mut authority)
            .map_err(|error| {
                CrossConeHirTypeAliasSurfaceError::TypeAliasInterfaces(Box::new(error))
            })?;
        Ok(TypeAliasValidatedCrossConeHirFrontSections(
            ValidatedSurfaceFront {
                graph,
                identities,
                foundations,
                hir_core_production,
                hir_interface,
                mir_core_production,
                lir_strong_production,
            },
        ))
    }
}

#[derive(Debug)]
pub enum CrossConeHirInternalClosureError {
    Interface(CrossConeHirInternalClosureValidationError),
}

impl std::fmt::Display for CrossConeHirInternalClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Interface(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeHirInternalClosureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Interface(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum CrossConeHirNominalSurfaceError {
    NominalInterfaces(
        Box<NominalInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirPropertySurfaceError {
    PropertyInterfaces(
        Box<PropertyInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirCallableSurfaceError {
    CallableInterfaces(
        Box<CallableInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

#[derive(Debug)]
pub enum CrossConeHirTypeAliasSurfaceError {
    TypeAliasInterfaces(
        Box<TypeAliasInterfaceSetSemanticValidationError<CrossConeHirNominalAuthorityError>>,
    ),
}

macro_rules! impl_surface_error {
    ($error:ident, $variant:ident) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::$variant(error) => error.fmt(formatter),
                }
            }
        }

        impl std::error::Error for $error {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Self::$variant(error) => Some(error),
                }
            }
        }
    };
}

impl_surface_error!(CrossConeHirNominalSurfaceError, NominalInterfaces);
impl_surface_error!(CrossConeHirPropertySurfaceError, PropertyInterfaces);
impl_surface_error!(CrossConeHirCallableSurfaceError, CallableInterfaces);
impl_surface_error!(CrossConeHirTypeAliasSurfaceError, TypeAliasInterfaces);
