use scoop_hir as hir;
use scoop_identity::{
    ConeCoordinate, ConeIdentity, PendingIdentityValidation, PersistentTypeAliasId,
    SemanticIdentitySession, SemanticOriginFingerprint,
};
use scoop_wire::{WirePath, decode_canonical, encode};

mod provider;

use provider::{ProviderFixture, package_path};

pub(super) struct DependencyWorldFixture {
    core: ProviderFixture,
    direct: Vec<ProviderFixture>,
    core_foundation: hir::ImportedHirFoundation,
    direct_foundations: Vec<hir::ImportedHirFoundation>,
    aliases: hir::CanonicalTypeAliasExpansionsV1,
}

impl DependencyWorldFixture {
    pub(super) fn with_nested_type(package: &[&str], outer: &str, nested: &str) -> Self {
        let direct = ProviderFixture::with_nested_type(
            ConeCoordinate::new("test", "dependency", "1.0.0").unwrap(),
            package_path(package),
            outer,
            nested,
        );
        Self::with_direct(vec![direct])
    }

    pub(super) fn with_conflicting_types(package: &[&str], name: &str) -> Self {
        let package = package_path(package);
        Self::with_direct(vec![
            ProviderFixture::with_nested_type(
                ConeCoordinate::new("test", "first-dependency", "1.0.0").unwrap(),
                package.clone(),
                name,
                "FirstNested",
            ),
            ProviderFixture::with_nested_type(
                ConeCoordinate::new("test", "second-dependency", "1.0.0").unwrap(),
                package,
                name,
                "SecondNested",
            ),
        ])
    }

    pub(super) fn with_distinct_types() -> Self {
        Self::with_direct(vec![
            ProviderFixture::with_nested_type(
                ConeCoordinate::new("test", "first-dependency", "1.0.0").unwrap(),
                package_path(&["first", "api"]),
                "First",
                "FirstNested",
            ),
            ProviderFixture::with_nested_type(
                ConeCoordinate::new("test", "second-dependency", "1.0.0").unwrap(),
                package_path(&["second", "api"]),
                "Second",
                "SecondNested",
            ),
        ])
    }

    pub(super) fn with_function_overloads(
        package: &[&str],
        name: &str,
        parameter_counts: [usize; 2],
    ) -> Self {
        let package = package_path(package);
        Self::with_direct(vec![
            ProviderFixture::with_function(
                ConeCoordinate::new("test", "first-dependency", "1.0.0").unwrap(),
                package.clone(),
                name,
                parameter_counts[0],
            ),
            ProviderFixture::with_function(
                ConeCoordinate::new("test", "second-dependency", "1.0.0").unwrap(),
                package,
                name,
                parameter_counts[1],
            ),
        ])
    }

    pub(super) fn with_object_and_function(
        package: &[&str],
        name: &str,
        parameter_count: usize,
    ) -> Self {
        Self::with_object_and_function_in_packages(package, package, name, parameter_count)
    }

    pub(super) fn with_object_and_function_in_packages(
        object_package: &[&str],
        function_package: &[&str],
        name: &str,
        parameter_count: usize,
    ) -> Self {
        Self::with_direct(vec![
            ProviderFixture::with_object(
                ConeCoordinate::new("test", "object-dependency", "1.0.0").unwrap(),
                package_path(object_package),
                name,
            ),
            ProviderFixture::with_function(
                ConeCoordinate::new("test", "function-dependency", "1.0.0").unwrap(),
                package_path(function_package),
                name,
                parameter_count,
            ),
        ])
    }

    fn with_direct(direct: Vec<ProviderFixture>) -> Self {
        let core = ProviderFixture::empty(ConeCoordinate::reserved_core());
        let mut session = SemanticIdentitySession::new();
        let core_foundation = import_foundation(&mut session, &core, 31);
        let direct_foundations = direct
            .iter()
            .enumerate()
            .map(|(index, provider)| {
                import_foundation(
                    &mut session,
                    provider,
                    37 + u8::try_from(index).expect("dependency fixture count fits u8"),
                )
            })
            .collect();
        Self {
            core,
            direct,
            core_foundation,
            direct_foundations,
            aliases: empty_alias_expansions(),
        }
    }

    pub(super) fn direct_identity(&self) -> ConeIdentity {
        self.direct[0].identity()
    }

    pub(super) fn world(&self, current: ConeIdentity) -> hir::ImportedSemanticWorld<'_> {
        hir::ImportedSemanticWorld::from_validated_closure(
            current,
            std::iter::once(hir::DirectImportedProviderInput::from_validated(
                certificate(self.core.coordinate(), 31),
                &self.core_foundation,
                self.core.interface(),
                &self.aliases,
            ))
            .chain(
                self.direct
                    .iter()
                    .zip(&self.direct_foundations)
                    .enumerate()
                    .map(|(index, (provider, foundation))| {
                        hir::DirectImportedProviderInput::from_validated(
                            certificate(
                                provider.coordinate(),
                                37 + u8::try_from(index).expect("dependency fixture count fits u8"),
                            ),
                            foundation,
                            provider.interface(),
                            &self.aliases,
                        )
                    }),
            )
            .collect(),
            Vec::new(),
        )
        .unwrap()
    }
}

fn import_foundation(
    session: &mut SemanticIdentitySession,
    fixture: &ProviderFixture,
    fingerprint: u8,
) -> hir::ImportedHirFoundation {
    let decoded: hir::DecodedHirFoundation =
        decode_canonical(&encode(fixture.foundation()).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(fixture.identity()).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let imported = session
        .import(
            fixture.identity(),
            SemanticOriginFingerprint::new(
                [fingerprint; 32],
                [fingerprint.wrapping_add(1); 32],
                [fingerprint.wrapping_add(2); 32],
            ),
            &identities,
        )
        .unwrap();
    let (hir, _, _) = imported.into_parts();
    hir::ImportedHirFoundation::from_odr_free(
        hir::OdrFreeHirFoundation::try_new(fixture.foundation().clone()).unwrap(),
        hir,
    )
}

fn empty_alias_expansions() -> hir::CanonicalTypeAliasExpansionsV1 {
    hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new())
        .unwrap()
        .expand_alias_closure(&EmptyAliasAuthority, &WirePath::root())
        .unwrap()
}

struct EmptyAliasAuthority;

impl hir::TypeAliasClosureAuthority for EmptyAliasAuthority {
    fn external_type_alias(
        &self,
        _alias: PersistentTypeAliasId,
    ) -> Option<&hir::TypeAliasInterfaceRecordV1> {
        None
    }

    fn is_type_alias_edge_authorized(
        &self,
        _source: PersistentTypeAliasId,
        _target: PersistentTypeAliasId,
    ) -> bool {
        false
    }
}

fn certificate(coordinate: &ConeCoordinate, fingerprint: u8) -> hir::ImportedProviderCertificate {
    hir::ImportedProviderCertificate::from_validated(
        coordinate.clone(),
        coordinate.identity().unwrap(),
        SemanticOriginFingerprint::new(
            [fingerprint; 32],
            [fingerprint.wrapping_add(1); 32],
            [fingerprint.wrapping_add(2); 32],
        ),
    )
}
