//! Complete semantic transactions start with independent source inventories.
use super::*;
use crate::cross_cone_type_semantics::inheritance::tests::support::Node;
use crate::cross_cone_type_semantics::protected_interfaces::tests::support::Fixture as SourceFixture;
use scoop_identity::*;
use scoop_wire::WirePath;
use std::collections::BTreeMap;

mod builtins;
mod closure;
mod defaults;
mod dispatch;
mod fixture;
mod foundation;
mod inventory;
mod members;
mod rejections;
mod selected;
mod uses;
use defaults::DefaultAuthority;
use fixture::*;
use uses::*;

fn path() -> WirePath {
    WirePath::root()
}

fn public() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
        Default::default(),
        Default::default(),
        Default::default(),
    )
}
fn public_proof(
    section: &CrossConeHirInterfaceSectionV1,
    provider: ConeIdentity,
) -> CheckedTypeSectionPublicSupportV1<'_> {
    CheckedTypeSectionPublicSupportV1::validate(
        section,
        provider,
        &CanonicalDirectPublicSurfaceV1::try_new(vec![]).unwrap(),
        &mut crate::cross_cone_interface::EmptyPublicSemanticAuthority(provider),
        &path(),
    )
    .unwrap()
}
fn check<'a>(
    fixture: &'a Fixture,
    candidate: &'a CrossConeTypeSemanticsSectionV1,
    public: &'a CrossConeHirInterfaceSectionV1,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    uses: &Uses,
) -> Result<
    CheckedCrossConeTypeSemanticsSectionV1<'a>,
    TypeSectionSemanticValidationError<&'static str>,
> {
    candidate.validate_semantics(
        public_proof(public, fixture.provider),
        dependencies,
        fixture,
        &mut fixture.source.clone(),
        &mut DefaultAuthority::new(fixture),
        uses,
        &path(),
    )
}
