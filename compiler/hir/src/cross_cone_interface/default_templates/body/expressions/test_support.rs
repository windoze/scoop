use std::sync::Arc;

use scoop_identity::{
    CallbackMode, CallbackParameterIndex, CallbackRegistrationKey, CanonicalIdentifier,
    ConeIdentity, DeclarationScope, DecodedPersistentId, DefinitionOrigin, DefinitionOwnerAtom,
    DefinitionOwnerChain, Effect, EnumVariantFieldKey, EnumVariantFieldSelector,
    EnumVariantIdentityKey, FieldIdentityKey, GeneratedCallableKey, InitializationUnitKey,
    LexicalCallableParent, LocalValueSelector, NormalizedSourcePath, OptionalSignatureType,
    PackagePath, PersistentCallbackRegistrationId, PersistentConstructorId,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentInitializationUnitId, PersistentKeyResolver,
    PersistentObjectValueId, PersistentPropertyAccessorId, PersistentPropertyId,
    PersistentSourceContextId, PersistentTypeId, SignatureCallableShape, SignatureTypeKey,
    SourceCAbiFunctionSignature, SourceCAbiReturn, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceNominalKind, SourceSpan, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};

use crate::{
    DefaultCallableDeclarationV1, DefaultCallableRefV1, ExportDefinitionSourceV1,
    TemplateLocalIndexResolver, TemplateLocalSelectorResolver,
};

pub(crate) struct Fixture {
    pub(crate) function: PersistentFunctionId,
    pub(crate) property: PersistentPropertyId,
    pub(crate) generated: PersistentGeneratedCallableId,
    pub(crate) type_id: PersistentTypeId,
    pub(crate) constructor: PersistentConstructorId,
    pub(crate) variant: PersistentEnumVariantId,
    pub(crate) variant_field: PersistentEnumVariantFieldId,
    pub(crate) field: PersistentFieldId,
    pub(crate) object: PersistentObjectValueId,
    pub(crate) callback: PersistentCallbackRegistrationId,
    pub(crate) initialization: PersistentInitializationUnitId,
}

impl Fixture {
    pub(crate) fn new() -> Self {
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
        let generated = PersistentGeneratedCallableId::from_key(
            &GeneratedCallableKey::CallableReferenceInvoke {
                parent: LexicalCallableParent::function(function),
                path: definition_path(),
            },
        )
        .unwrap();
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
        let field = PersistentFieldId::from_key(
            &FieldIdentityKey::source_declared(&structure, identifier("value")).unwrap(),
        )
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
        let variant_field = PersistentEnumVariantFieldId::from_key(&EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ))
        .unwrap();
        let object_key = SourceDeclarationKey::nominal(
            top_level_site(),
            identifier("Singleton"),
            SourceNominalKind::Object,
            0,
        );
        let object = PersistentObjectValueId::from_source_object(&object_key).unwrap();
        let callback = PersistentCallbackRegistrationId::from_key(&CallbackRegistrationKey::new(
            LexicalCallableParent::function(function),
            StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
                [],
            ),
            SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
            CallbackParameterIndex::new(0),
            SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                SignatureTypeKey::Binder { depth: 0, index: 0 },
            ),
            CallbackMode::Reusable,
        ))
        .unwrap();
        let initialization = PersistentInitializationUnitId::from_key(
            &InitializationUnitKey::TopLevelProperty(property),
        )
        .unwrap();
        Self {
            function,
            property,
            generated,
            type_id: structure_id,
            constructor,
            variant,
            variant_field,
            field,
            object,
            callback,
            initialization,
        }
    }

    pub(crate) fn callable(&self) -> DefaultCallableRefV1 {
        DefaultCallableRefV1::try_new(
            DefaultCallableDeclarationV1::Function(self.function),
            OptionalSignatureType::Absent,
            Vec::new(),
        )
        .unwrap()
    }

    pub(crate) fn origin(&self) -> ExportDefinitionSourceV1 {
        origin()
    }

    pub(crate) const fn value_type(&self) -> SignatureTypeKey {
        SignatureTypeKey::Binder { depth: 0, index: 0 }
    }

    pub(crate) const fn local(&self) -> LocalValueSelector {
        LocalValueSelector::Parameter {
            declaration_index: 0,
        }
    }

    pub(crate) fn locals(&self) -> LocalResolver {
        LocalResolver {
            selector: self.local(),
        }
    }

    pub(crate) const fn resolver(&self) -> Resolver {
        Resolver {
            function: Some(self.function),
            property: Some(self.property),
            generated: Some(self.generated),
            type_id: Some(self.type_id),
            constructor: Some(self.constructor),
            variant: Some(self.variant),
            variant_field: Some(self.variant_field),
            field: Some(self.field),
            object: Some(self.object),
            initialization: Some(self.initialization),
        }
    }
}

pub(crate) struct Resolver {
    function: Option<PersistentFunctionId>,
    property: Option<PersistentPropertyId>,
    generated: Option<PersistentGeneratedCallableId>,
    type_id: Option<PersistentTypeId>,
    constructor: Option<PersistentConstructorId>,
    variant: Option<PersistentEnumVariantId>,
    variant_field: Option<PersistentEnumVariantFieldId>,
    field: Option<PersistentFieldId>,
    object: Option<PersistentObjectValueId>,
    initialization: Option<PersistentInitializationUnitId>,
}

impl Resolver {
    pub(crate) const fn rejecting() -> Self {
        Self {
            function: None,
            property: None,
            generated: None,
            type_id: None,
            constructor: None,
            variant: None,
            variant_field: None,
            field: None,
            object: None,
            initialization: None,
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
reject_identity!(PersistentPropertyAccessorId);
reject_identity!(PersistentGenericTypeId);
reject_identity!(PersistentCallbackRegistrationId);

impl PersistentIdResolver<PersistentConstructorId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentConstructorId>,
    ) -> Result<PersistentConstructorId, Self::Error> {
        id.verify(self.constructor.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentFieldId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentFieldId>,
    ) -> Result<PersistentFieldId, Self::Error> {
        id.verify(self.field.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        id.verify(self.type_id.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentObjectValueId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentObjectValueId>,
    ) -> Result<PersistentObjectValueId, Self::Error> {
        id.verify(self.object.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentGeneratedCallableId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGeneratedCallableId>,
    ) -> Result<PersistentGeneratedCallableId, Self::Error> {
        id.verify(self.generated.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentEnumVariantId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentEnumVariantId>,
    ) -> Result<PersistentEnumVariantId, Self::Error> {
        id.verify(self.variant.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentEnumVariantFieldId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentEnumVariantFieldId>,
    ) -> Result<PersistentEnumVariantFieldId, Self::Error> {
        id.verify(self.variant_field.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<PersistentInitializationUnitId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentInitializationUnitId>,
    ) -> Result<PersistentInitializationUnitId, Self::Error> {
        id.verify(self.initialization.ok_or(ResolutionError)?)
            .map_err(|_| ResolutionError)
    }
}

impl PersistentIdResolver<ConeIdentity> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<ConeIdentity>,
    ) -> Result<ConeIdentity, Self::Error> {
        id.verify(ConeIdentity::CORE).map_err(|_| ResolutionError)
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
            .map_err(|_| ResolutionError)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolutionError;

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("identity is absent")
    }
}

impl std::error::Error for ResolutionError {}

pub(crate) struct LocalResolver {
    selector: LocalValueSelector,
}

impl TemplateLocalSelectorResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_selector(
        &mut self,
        index: u32,
    ) -> Result<LocalValueSelector, Self::Error> {
        if index == 0 {
            Ok(self.selector.clone())
        } else {
            Err(LocalError)
        }
    }
}

impl TemplateLocalIndexResolver for LocalResolver {
    type Error = LocalError;

    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        if selector == &self.selector {
            Ok(0)
        } else {
            Err(LocalError)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LocalError;

impl std::fmt::Display for LocalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("local is absent")
    }
}

impl std::error::Error for LocalError {}

fn origin() -> ExportDefinitionSourceV1 {
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(
            source_identity(),
            SourceSpan::new(2, 5).unwrap(),
            &context_key(),
        )
        .unwrap(),
    )
}

fn source_identity() -> SourceIdentity {
    SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Expressions.scoop").unwrap(),
    )
    .unwrap()
}

fn context_key() -> SourceContextKey {
    SourceContextKey::File {
        source: source_identity(),
    }
}

pub(crate) fn definition_path() -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(
        StructuralPathSegment::new(StructuralDefinitionSiteRole::CallableConversion, 0),
        [],
    )
}

fn top_level_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn owned_site(owner: DefinitionOwnerAtom) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(vec![owner]),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
