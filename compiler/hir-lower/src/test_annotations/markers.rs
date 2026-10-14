use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, PackagePath,
    PersistentAnnotationId, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum TestMarker {
    Test,
    Before,
    After,
    Ignore,
}

impl TestMarker {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Test => "Test",
            Self::Before => "BeforeTest",
            Self::After => "AfterTest",
            Self::Ignore => "Ignore",
        }
    }

    pub(super) fn from_identity(identity: PersistentAnnotationId) -> Option<Self> {
        static MARKERS: OnceLock<[(PersistentAnnotationId, TestMarker); 4]> = OnceLock::new();
        MARKERS
            .get_or_init(|| {
                let cone = ConeCoordinate::new("scoop", "scoop.test", "0.1.0")
                    .unwrap()
                    .identity()
                    .unwrap();
                [Self::Test, Self::Before, Self::After, Self::Ignore].map(|marker| {
                    let key = SourceDeclarationKey::nominal(
                        SourceDeclarationSite::new(
                            cone,
                            PackagePath::from_segments(vec![
                                CanonicalIdentifier::new("scoop").unwrap(),
                                CanonicalIdentifier::new("test").unwrap(),
                            ]),
                            DefinitionOwnerChain::top_level(),
                            DeclarationScope::ConeWide,
                        )
                        .unwrap(),
                        CanonicalIdentifier::new(marker.name()).unwrap(),
                        SourceNominalKind::AnnotationClass,
                        0,
                    );
                    (
                        PersistentAnnotationId::from_source_declaration(&key).unwrap(),
                        marker,
                    )
                })
            })
            .iter()
            .find_map(|&(id, marker)| (identity == id).then_some(marker))
    }
}
