use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOwnerChain,
    OptionalSignatureType, PackagePath, PersistentConstructorId, PersistentEnumVariantId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, SourceDeclarationKey, SourceDeclarationSite,
};

use crate::{DefaultCallableDeclarationV1, DefaultCallableRefV1};

pub(super) struct Fixture {
    pub(super) function: PersistentFunctionId,
    pub(super) property: PersistentPropertyId,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                top_level_site(),
                identifier("integerOperation"),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let property = PersistentPropertyId::from_source_declaration(
            &SourceDeclarationKey::property(top_level_site(), identifier("message")),
        )
        .unwrap();
        Self { function, property }
    }

    pub(super) fn callable(&self) -> DefaultCallableRefV1 {
        DefaultCallableRefV1::try_new(
            DefaultCallableDeclarationV1::Function(self.function),
            OptionalSignatureType::Absent,
            Vec::new(),
        )
        .unwrap()
    }

    pub(super) const fn resolver(&self) -> Resolver {
        Resolver {
            function: Some(self.function),
            property: Some(self.property),
        }
    }
}

pub(super) struct Resolver {
    function: Option<PersistentFunctionId>,
    property: Option<PersistentPropertyId>,
}

impl Resolver {
    pub(super) const fn rejecting() -> Self {
        Self {
            function: None,
            property: None,
        }
    }
}

impl PersistentIdResolver<PersistentFunctionId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<PersistentFunctionId, Self::Error> {
        id.verify(self.function.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentPropertyId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentPropertyId>,
    ) -> Result<PersistentPropertyId, Self::Error> {
        id.verify(self.property.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

macro_rules! reject_identity {
    ($identity:ty) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                _id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                Err(ResolutionError)
            }
        }
    };
}

reject_identity!(PersistentGenericFunctionId);
reject_identity!(PersistentConstructorId);
reject_identity!(PersistentPropertyAccessorId);
reject_identity!(PersistentEnumVariantId);
reject_identity!(PersistentGeneratedCallableId);
reject_identity!(PersistentTypeId);
reject_identity!(PersistentGenericTypeId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent")
    }
}

impl std::error::Error for ResolutionError {}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
