use std::sync::Arc;

use scoop_identity::{
    AccessorRole, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOrigin, DefinitionOwnerChain, IdentityReferenceError,
    NormalizedSourcePath, PackagePath, PendingIdentityValidation, PersistentFunctionId,
    PersistentId, PersistentIdResolver, PersistentKeyResolver, PersistentPropertyAccessorId,
    PersistentSourceContextId, PropertyAccessorKey, PropertyOwner, SignatureTypeKey,
    SourceContextKey, SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceSpan,
    ValidatedIdentityGraph,
};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::*;
use crate::{
    CallableParameterCallingResolutionError, CallableParameterCallingV1,
    CallableSourceParameterResolutionError, CallableSourceParameterV1, ExportDefinitionSourceV1,
};

#[test]
fn indexed_record_has_fixed_shape_and_round_trips_to_semantic_keys() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let mut template_indices = TemplateIndexResolver::new(fixture.templates.clone());
    let indexed = record.index_templates(&mut template_indices).unwrap();
    let bytes = encode(&indexed).unwrap();
    assert_eq!(
        hex(&bytes),
        "a201a20001015820af418bd2075fe80f10ad47ef0fea34845c20891a5ca83271b942d98a557321560283a40168726571756972656402a300070100020003a1000104a301a20158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02717372632f436f6c6c6563742e73636f6f7002a20104020b035820453de8be5cb869f5f1f9434dce1c9e33e942ee5d7089d92b70de600cb847050aa4016866616c6c6261636b02a300070100020103a20002010004a301a20158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02717372632f436f6c6c6563742e73636f6f7002a20104020b035820453de8be5cb869f5f1f9434dce1c9e33e942ee5d7089d92b70de600cb847050aa401647461696c02a300070100020203a3000401a3000701000203020104a301a20158205ea5f5e8ff248182c8f8c7e1043caae20f163bcefd34cca4e97d8c6a03bf620d02717372632f436f6c6c6563742e73636f6f7002a20104020b035820453de8be5cb869f5f1f9434dce1c9e33e942ee5d7089d92b70de600cb847050a"
    );

    let decoded: DecodedCallableSourceInterfaceV1 = decode_canonical(&bytes).unwrap();
    let mut identities = IdentityResolver(fixture.identities());
    let mut template_keys = TemplateKeyResolver::new(fixture.templates.clone());
    assert_eq!(
        decoded
            .resolve(&mut identities, &mut template_keys)
            .unwrap(),
        record
    );
}

#[test]
fn record_rejects_accessor_owners_and_mismatched_template_keys() {
    let fixture = Fixture::new();
    let parameters = CanonicalCallableSourceParametersV1::try_new(Vec::new()).unwrap();
    let property =
        CborIdentityRecord::from_key(SourceDeclarationKey::property(site(), identifier("value")))
            .unwrap();
    let accessor = PersistentPropertyAccessorId::from_key(&PropertyAccessorKey::new(
        PropertyOwner::Property(property.id()),
        AccessorRole::Getter,
    ))
    .unwrap();
    assert_eq!(
        CallableSourceInterfaceV1::try_new(CallableTemplateOrigin::Accessor(accessor), parameters,),
        Err(CallableSourceInterfaceBuildError::PropertyAccessorOwner)
    );

    let actual = ExportDefaultTemplateKeyV1::new(fixture.owner, 0);
    let parameters = CanonicalCallableSourceParametersV1::try_new(vec![parameter(
        "value",
        binder(0),
        CallableParameterCallingV1::Default { template: actual },
        fixture.origin.clone(),
    )])
    .unwrap();
    let different_owner = CallableTemplateOrigin::Function(function("other").id());
    assert!(matches!(
        CallableSourceInterfaceV1::try_new(different_owner, parameters),
        Err(CallableSourceInterfaceBuildError::TemplateKey {
            position: 0,
            expected,
            actual: found,
        }) if expected == ExportDefaultTemplateKeyV1::new(different_owner, 0) && found == actual
    ));
}

#[test]
fn decoding_reports_missing_template_index_and_owner_authority() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let mut indices = TemplateIndexResolver::new(fixture.templates.clone());
    let bytes = encode(&record.index_templates(&mut indices).unwrap()).unwrap();
    let decoded: DecodedCallableSourceInterfaceV1 = decode_canonical(&bytes).unwrap();
    let mut empty_identities = IdentityResolver(PendingIdentityValidation::new().finish().unwrap());
    let mut templates = TemplateKeyResolver::new(fixture.templates.clone());
    assert!(matches!(
        decoded.resolve(&mut empty_identities, &mut templates),
        Err(CallableSourceInterfaceResolutionError::Owner(
            IdentityReferenceError::Missing { .. }
        ))
    ));

    let decoded: DecodedCallableSourceInterfaceV1 = decode_canonical(&bytes).unwrap();
    let mut identities = IdentityResolver(fixture.identities());
    let mut missing = TemplateKeyResolver::new(vec![fixture.templates[0]]);
    assert!(matches!(
        decoded.resolve(&mut identities, &mut missing),
        Err(CallableSourceInterfaceResolutionError::Parameters(
            CallableSourceParameterListResolutionError::Parameter {
                index: 2,
                error: CallableSourceParameterResolutionError::Calling(
                    CallableParameterCallingResolutionError::Template(
                        TemplateLookupError::MissingIndex(1)
                    )
                )
            }
        ))
    ));
}

#[test]
fn indexing_reports_the_parameter_with_a_missing_template_key() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let mut missing = TemplateIndexResolver::new(vec![fixture.templates[0]]);

    assert!(matches!(
        record.index_templates(&mut missing),
        Err(CallableSourceInterfaceIndexError::Parameter {
            index: 2,
            error: TemplateLookupError::MissingKey(key),
        }) if key == fixture.templates[1]
    ));
}

#[test]
fn decoder_requires_exact_record_shape() {
    let error =
        decode_canonical::<DecodedCallableSourceInterfaceV1>(&[0xa1, 0x01, 0x00]).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}

struct Fixture {
    function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>,
    owner: CallableTemplateOrigin,
    context: CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    origin: ExportDefinitionSourceV1,
    templates: Vec<ExportDefaultTemplateKeyV1>,
}

impl Fixture {
    fn new() -> Self {
        let function = function("collect");
        let owner = CallableTemplateOrigin::Function(function.id());
        let source = SourceIdentity::new(
            ConeIdentity::CORE,
            NormalizedSourcePath::new("src/Collect.scoop").unwrap(),
        )
        .unwrap();
        let context = CborIdentityRecord::from_key(SourceContextKey::File {
            source: source.clone(),
        })
        .unwrap();
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(4, 11).unwrap(), context.key()).unwrap(),
        );
        Self {
            function,
            owner,
            context,
            origin,
            templates: vec![
                ExportDefaultTemplateKeyV1::new(owner, 1),
                ExportDefaultTemplateKeyV1::new(owner, 2),
            ],
        }
    }

    fn record(&self) -> CallableSourceInterfaceV1 {
        CallableSourceInterfaceV1::try_new(
            self.owner,
            CanonicalCallableSourceParametersV1::try_new(vec![
                parameter(
                    "required",
                    binder(0),
                    CallableParameterCallingV1::Required,
                    self.origin.clone(),
                ),
                parameter(
                    "fallback",
                    binder(1),
                    CallableParameterCallingV1::Default {
                        template: self.templates[0],
                    },
                    self.origin.clone(),
                ),
                parameter(
                    "tail",
                    binder(2),
                    CallableParameterCallingV1::VarargDefault {
                        element_type: binder(3),
                        template: self.templates[1],
                    },
                    self.origin.clone(),
                ),
            ])
            .unwrap(),
        )
        .unwrap()
    }

    fn identities(&self) -> ValidatedIdentityGraph {
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        pending
            .register_external_canonical_authority(self.function.clone())
            .unwrap();
        pending
            .register_external_canonical_authority(self.context.clone())
            .unwrap();
        pending.finish().unwrap()
    }
}

fn parameter(
    name: &str,
    value_type: SignatureTypeKey,
    calling: CallableParameterCallingV1,
    origin: ExportDefinitionSourceV1,
) -> CallableSourceParameterV1 {
    CallableSourceParameterV1::new(identifier(name), value_type, calling, origin)
}

fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}

fn function(name: &str) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        site(),
        identifier(name),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

struct IdentityResolver(ValidatedIdentityGraph);

impl<I> PersistentIdResolver<I> for IdentityResolver
where
    I: PersistentId,
    ValidatedIdentityGraph: PersistentIdResolver<I, Error = IdentityReferenceError>,
{
    type Error = IdentityReferenceError;

    fn resolve(&mut self, id: scoop_identity::DecodedPersistentId<I>) -> Result<I, Self::Error> {
        self.0.resolve(id)
    }
}

impl<I, K> PersistentKeyResolver<I, K> for IdentityResolver
where
    I: PersistentId,
    ValidatedIdentityGraph: PersistentKeyResolver<I, K, Error = IdentityReferenceError>,
{
    type Error = IdentityReferenceError;

    fn resolve_key(
        &mut self,
        id: scoop_identity::DecodedPersistentId<I>,
    ) -> Result<Arc<K>, Self::Error> {
        self.0.resolve_key(id)
    }
}

struct TemplateKeyResolver {
    keys: Vec<ExportDefaultTemplateKeyV1>,
}

impl TemplateKeyResolver {
    fn new(keys: Vec<ExportDefaultTemplateKeyV1>) -> Self {
        Self { keys }
    }
}

impl ExportDefaultTemplateKeyResolver for TemplateKeyResolver {
    type Error = TemplateLookupError;

    fn resolve_default_template_key(
        &mut self,
        index: u32,
    ) -> Result<ExportDefaultTemplateKeyV1, Self::Error> {
        self.keys
            .get(index as usize)
            .copied()
            .ok_or(TemplateLookupError::MissingIndex(index))
    }
}

struct TemplateIndexResolver {
    keys: Vec<ExportDefaultTemplateKeyV1>,
}

impl TemplateIndexResolver {
    fn new(keys: Vec<ExportDefaultTemplateKeyV1>) -> Self {
        Self { keys }
    }
}

impl ExportDefaultTemplateIndexResolver for TemplateIndexResolver {
    type Error = TemplateLookupError;

    fn resolve_default_template_index(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Result<u32, Self::Error> {
        self.keys
            .iter()
            .position(|candidate| *candidate == key)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or(TemplateLookupError::MissingKey(key))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TemplateLookupError {
    MissingIndex(u32),
    MissingKey(ExportDefaultTemplateKeyV1),
}

impl std::fmt::Display for TemplateLookupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TemplateLookupError {}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
