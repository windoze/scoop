use std::sync::Arc;

use scoop_identity::{
    AccessorRole, CanonicalIdentifier, ConeIdentity, DeclarationScope, DecodedPersistentId,
    DefinitionOrigin, DefinitionOwnerAtom, DefinitionOwnerChain, Effect, EnumVariantIdentityKey,
    GeneratedCallableKey, LexicalCallableParent, LexicalCallableRole, LocalValueSelector,
    NormalizedSourcePath, PackagePath, PersistentConstructorId, PersistentEnumVariantId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver, PersistentKeyResolver,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentSourceContextId,
    PersistentTypeId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

use crate::{
    DefaultCaptureV1, ExportDefinitionSourceV1, TemplateLocalIndexResolver,
    TemplateLocalSelectorResolver,
};

pub(super) struct Fixture {
    pub(super) function: PersistentFunctionId,
    pub(super) constructor: PersistentConstructorId,
    pub(super) lambda_body: PersistentGeneratedCallableId,
    pub(super) anonymous_body: PersistentGeneratedCallableId,
    accessor: PersistentPropertyAccessorId,
    variant: PersistentEnumVariantId,
}

impl Fixture {
    pub(super) fn new() -> Self {
        let function = source_function("root");
        let structure = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Record"),
            SourceNominalKind::Struct,
            0,
        );
        let structure_id = PersistentTypeId::from_source_declaration(&structure).unwrap();
        let constructor =
            PersistentConstructorId::from_source_declaration(&SourceDeclarationKey::constructor(
                owned_site(DefinitionOwnerAtom::Type(structure_id)),
                Vec::new(),
            ))
            .unwrap();
        let property = PersistentPropertyId::from_source_declaration(
            &SourceDeclarationKey::property(top_level_site(), identifier("value")),
        )
        .unwrap();
        let accessor = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
            PropertyOwner::Property(property),
            AccessorRole::Getter,
        ))
        .unwrap();
        let enumeration = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Choice"),
            SourceNominalKind::Enum,
            0,
        );
        let variant = PersistentEnumVariantId::from_key(
            &EnumVariantIdentityKey::source(&enumeration, identifier("Only")).unwrap(),
        )
        .unwrap();
        Self {
            function,
            constructor,
            lambda_body: lexical_body(function, LexicalCallableRole::LambdaBody, 0),
            anonymous_body: lexical_body(function, LexicalCallableRole::AnonymousFunctionBody, 1),
            accessor,
            variant,
        }
    }

    pub(super) fn resolver(&self) -> Resolver {
        Resolver {
            function: self.function,
            constructor: self.constructor,
            generated: [self.lambda_body, self.anonymous_body],
            accessor: self.accessor,
            variant: self.variant,
        }
    }

    pub(super) fn capture(&self, declaration_index: u32) -> DefaultCaptureV1 {
        DefaultCaptureV1::new(
            parameter(declaration_index),
            binder(declaration_index),
            origin(),
        )
    }
}

pub(super) struct Resolver {
    function: PersistentFunctionId,
    constructor: PersistentConstructorId,
    generated: [PersistentGeneratedCallableId; 2],
    accessor: PersistentPropertyAccessorId,
    variant: PersistentEnumVariantId,
}

macro_rules! resolve_fixture_identity {
    ($identity:ty, $field:ident) => {
        impl PersistentIdResolver<$identity> for Resolver {
            type Error = ResolutionError;

            fn resolve(
                &mut self,
                id: DecodedPersistentId<$identity>,
            ) -> Result<$identity, Self::Error> {
                id.verify(self.$field).map_err(|_| ResolutionError)
            }
        }
    };
}

resolve_fixture_identity!(PersistentFunctionId, function);
resolve_fixture_identity!(PersistentConstructorId, constructor);
resolve_fixture_identity!(PersistentPropertyAccessorId, accessor);
resolve_fixture_identity!(PersistentEnumVariantId, variant);

impl PersistentIdResolver<PersistentGeneratedCallableId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGeneratedCallableId>,
    ) -> Result<PersistentGeneratedCallableId, Self::Error> {
        self.generated
            .into_iter()
            .find(|candidate| candidate.as_array() == id.as_array())
            .ok_or(ResolutionError)
    }
}

impl PersistentIdResolver<PersistentGenericFunctionId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<PersistentGenericFunctionId>,
    ) -> Result<PersistentGenericFunctionId, Self::Error> {
        Err(ResolutionError)
    }
}

impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        Err(ResolutionError)
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        Err(ResolutionError)
    }
}

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE)
            .map_err(|_: PersistentIdMismatch<ConeIdentity>| ResolutionError)
    }
}

impl PersistentKeyResolver<PersistentSourceContextId, SourceContextKey> for Resolver {
    type Error = ResolutionError;

    fn resolve_key(
        &mut self,
        id: DecodedPersistentId<PersistentSourceContextId>,
    ) -> Result<Arc<SourceContextKey>, Self::Error> {
        let key = context_key();
        id.verify(PersistentSourceContextId::from_key(&key).unwrap())
            .map(|_| Arc::new(key))
            .map_err(|_: PersistentIdMismatch<PersistentSourceContextId>| ResolutionError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent from the nested-callable fixture")
    }
}

impl std::error::Error for ResolutionError {}

pub(super) struct LocalResolver {
    selectors: Vec<LocalValueSelector>,
}

impl LocalResolver {
    pub(super) const fn new(selectors: Vec<LocalValueSelector>) -> Self {
        Self { selectors }
    }
}

impl TemplateLocalSelectorResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_selector(
        &mut self,
        index: u32,
    ) -> Result<LocalValueSelector, Self::Error> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.selectors.get(index))
            .cloned()
            .ok_or(LocalError::MissingIndex(index))
    }
}

impl TemplateLocalIndexResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        self.selectors
            .iter()
            .position(|candidate| candidate == selector)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or_else(|| LocalError::MissingSelector(selector.clone()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LocalError {
    MissingIndex(u32),
    MissingSelector(LocalValueSelector),
}

impl std::fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LocalError {}

pub(super) fn source_function(name: &str) -> PersistentFunctionId {
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        top_level_site(),
        identifier(name),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

pub(super) fn lexical_body(
    parent: PersistentFunctionId,
    role: LexicalCallableRole,
    ordinal: u32,
) -> PersistentGeneratedCallableId {
    PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Lexical {
        parent: LexicalCallableParent::function(parent),
        role,
        path: path(StructuralDefinitionSiteRole::Lambda, ordinal),
    })
    .unwrap()
}

pub(super) fn path(role: StructuralDefinitionSiteRole, ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(StructuralPathSegment::new(role, ordinal), [])
}

pub(super) const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

pub(super) fn function_type() -> SignatureTypeKey {
    SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![binder(0)],
        result: Box::new(binder(1)),
    }
}

pub(super) fn parameter(declaration_index: u32) -> LocalValueSelector {
    LocalValueSelector::Parameter { declaration_index }
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = source_identity();
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(2, 5).unwrap(), &context_key()).unwrap(),
    )
}

fn source_identity() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Nested.scoop").unwrap(),
    )
    .unwrap()
}

fn context_key() -> SourceContextKey {
    SourceContextKey::File {
        source: source_identity(),
    }
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

impl PersistentIdResolver<scoop_identity::PersistentExactTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        _id: DecodedPersistentId<scoop_identity::PersistentExactTypeId>,
    ) -> Result<scoop_identity::PersistentExactTypeId, Self::Error> {
        Err(ResolutionError)
    }
}
