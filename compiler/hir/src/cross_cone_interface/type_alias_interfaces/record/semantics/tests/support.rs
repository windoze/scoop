use std::collections::BTreeMap;

use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, NormalizedSourcePath, PackagePath, PersistentGenericTypeId,
    PersistentTypeAliasId, PersistentTypeId, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
};

use super::super::*;
use crate::{
    ExportDefinitionSourceV1, NominalInterfaceShapeAuthority, PublicLookupAccessV1,
    PublicNominalKindV1, PublicNominalShapeV1, TypeAliasTargetV1,
};

pub(super) struct Fixture {
    pub(super) alias: PersistentTypeAliasId,
    pub(super) alias_key: SourceDeclarationKey,
    pub(super) other_alias: PersistentTypeAliasId,
    pub(super) target: PersistentTypeId,
    pub(super) origin: ExportDefinitionSourceV1,
    source: TypeAliasDeclarationSourceV1,
    concrete_nominals: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let declaration = alias_key("Alias", top_level_site(ConeIdentity::CORE));
        let alias = PersistentTypeAliasId::from_source_declaration(&declaration).unwrap();
        let other_alias = PersistentTypeAliasId::from_source_declaration(&alias_key(
            "Other",
            top_level_site(ConeIdentity::CORE),
        ))
        .unwrap();
        let target = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
            top_level_site(ConeIdentity::CORE),
            identifier("Target"),
            SourceNominalKind::Struct,
            0,
        ))
        .unwrap();
        let origin = definition_origin(source(ConeIdentity::CORE, "src/Alias.scoop"), 0, 10);
        let source = TypeAliasDeclarationSourceV1::new(
            declaration.clone(),
            PublicLookupAccessV1::DirectOnly,
            origin.origin().clone(),
        );
        Self {
            alias,
            alias_key: declaration,
            other_alias,
            target,
            origin,
            source,
            concrete_nominals: BTreeMap::from([(
                target,
                PublicNominalShapeV1::new(PublicNominalKindV1::Struct, 0),
            )]),
        }
    }

    pub(super) fn record(&self) -> TypeAliasInterfaceRecordV1 {
        self.record_with(
            TypeAliasTargetV1::Signature(scoop_identity::SignatureTypeKey::Nominal(self.target)),
            self.origin.clone(),
        )
    }

    pub(super) fn record_with(
        &self,
        target: TypeAliasTargetV1,
        origin: ExportDefinitionSourceV1,
    ) -> TypeAliasInterfaceRecordV1 {
        TypeAliasInterfaceRecordV1::try_new(
            self.alias,
            target,
            PublicLookupAccessV1::DirectOnly,
            origin,
        )
        .unwrap()
    }

    pub(super) fn authority(&self) -> TestAuthority {
        TestAuthority {
            current_cone: ConeIdentity::CORE,
            alias: self.alias,
            source: self.source.clone(),
            concrete_nominals: self.concrete_nominals.clone(),
            generic_nominals: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestAuthorityError {
    Alias(PersistentTypeAliasId),
    Concrete(PersistentTypeId),
    Generic(PersistentGenericTypeId),
}

impl std::fmt::Display for TestAuthorityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing test authority: {self:?}")
    }
}

impl std::error::Error for TestAuthorityError {}

pub(super) struct TestAuthority {
    pub(super) current_cone: ConeIdentity,
    pub(super) alias: PersistentTypeAliasId,
    pub(super) source: TypeAliasDeclarationSourceV1,
    pub(super) concrete_nominals: BTreeMap<PersistentTypeId, PublicNominalShapeV1>,
    pub(super) generic_nominals: BTreeMap<PersistentGenericTypeId, PublicNominalShapeV1>,
}

impl NominalInterfaceShapeAuthority<TestAuthorityError> for TestAuthority {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.concrete_nominals
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Concrete(declaration))
    }

    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, TestAuthorityError> {
        self.generic_nominals
            .get(&declaration)
            .copied()
            .ok_or(TestAuthorityError::Generic(declaration))
    }
}

impl TypeAliasInterfaceSemanticAuthority<TestAuthorityError> for TestAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.current_cone
    }

    fn type_alias_declaration_source(
        &mut self,
        alias: PersistentTypeAliasId,
    ) -> Result<TypeAliasDeclarationSourceV1, TestAuthorityError> {
        if alias == self.alias {
            Ok(self.source.clone())
        } else {
            Err(TestAuthorityError::Alias(alias))
        }
    }
}

pub(super) fn alias_key(name: &str, site: SourceDeclarationSite) -> SourceDeclarationKey {
    SourceDeclarationKey::type_alias(site, identifier(name))
}

pub(super) fn generic_nominal(name: &str, arity: u32) -> PersistentGenericTypeId {
    PersistentGenericTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        top_level_site(ConeIdentity::CORE),
        identifier(name),
        SourceNominalKind::Class,
        arity,
    ))
    .unwrap()
}

pub(super) fn top_level_site(cone: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

pub(super) fn definition_origin(
    source: SourceIdentity,
    start: u64,
    end: u64,
) -> ExportDefinitionSourceV1 {
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(start, end).unwrap(), &context).unwrap(),
    )
}

pub(super) fn source(cone: ConeIdentity, path: &str) -> SourceIdentity {
    SourceIdentity::new(cone, NormalizedSourcePath::new(path).unwrap()).unwrap()
}

pub(super) fn foreign_cone() -> ConeIdentity {
    ConeCoordinate::new("example", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

pub(super) fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
