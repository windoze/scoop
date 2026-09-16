use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOrigin, DefinitionOwnerChain, NormalizedSourcePath, PackagePath,
    PendingIdentityValidation, PersistentFunctionId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceSpan,
    ValidatedIdentityGraph,
};
use scoop_wire::{DecodeLimits, Encoder, WireEncode, decode_canonical, encode};

use super::*;
use crate::{
    CallableParameterCallingV1, CallableSourceParameterV1, CanonicalCallableSourceParametersV1,
    ExportDefaultTemplateKeyV1, ExportDefinitionSourceV1,
};

#[test]
fn producer_sorts_rejects_duplicates_and_supports_typed_lookup() {
    let first = empty_record("alpha");
    let second = empty_record("omega");
    let (first, second) = ordered(first, second);
    let table =
        CanonicalCallableSourceInterfacesV1::try_new(vec![second.clone(), first.clone()]).unwrap();

    assert_eq!(table.records(), &[first.clone(), second.clone()]);
    assert_eq!(table.get(first.owner()), Some(&first));
    assert_eq!(table.get(second.owner()), Some(&second));
    assert!(!table.is_empty());
    assert_eq!(
        CanonicalCallableSourceInterfacesV1::try_new(vec![first.clone(), first]),
        Err(CallableSourceInterfaceSetBuildError::DuplicateOwner(
            table.records()[0].owner()
        ))
    );
}

#[test]
fn indexed_table_has_canonical_wire_and_decodes_in_order() {
    let first_record = empty_record("first");
    let second_record = empty_record("second");
    let (first, second) = ordered(first_record, second_record);
    let table =
        CanonicalCallableSourceInterfacesV1::try_new(vec![second.clone(), first.clone()]).unwrap();
    let mut indices = NoTemplates;
    let bytes = encode(&table.index_templates(&mut indices).unwrap()).unwrap();

    let mut first_indices = NoTemplates;
    let first_bytes = encode(&first.index_templates(&mut first_indices).unwrap()).unwrap();
    let mut second_indices = NoTemplates;
    let second_bytes = encode(&second.index_templates(&mut second_indices).unwrap()).unwrap();
    assert_eq!(
        bytes,
        [
            b"\x82".as_slice(),
            first_bytes.as_slice(),
            second_bytes.as_slice()
        ]
        .concat()
    );

    let decoded: DecodedCanonicalCallableSourceInterfacesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut identities = identities([first.owner(), second.owner()]);
    let mut templates = NoTemplates;
    assert_eq!(
        decoded.resolve(&mut identities, &mut templates).unwrap(),
        table
    );
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_owner_order() {
    let left = empty_record("left");
    let right = empty_record("right");
    let (first, second) = ordered(left, right);

    let duplicate = decode_table(&SourceInterfaceSequence(vec![first.clone(), first.clone()]));
    let mut duplicate_identities = identities([first.owner()]);
    let mut templates = NoTemplates;
    assert!(matches!(
        duplicate.resolve(&mut duplicate_identities, &mut templates),
        Err(CallableSourceInterfaceSetValidationError::DuplicateOwner {
            index: 1,
            owner,
        }) if owner == first.owner()
    ));

    let reversed = decode_table(&SourceInterfaceSequence(vec![
        second.clone(),
        first.clone(),
    ]));
    let mut reversed_identities = identities([first.owner(), second.owner()]);
    let mut templates = NoTemplates;
    assert!(matches!(
        reversed.resolve(&mut reversed_identities, &mut templates),
        Err(CallableSourceInterfaceSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn table_indexing_reports_record_and_parameter_location() {
    let function = function("defaulted");
    let owner = CallableTemplateOrigin::Function(function.id());
    let key = ExportDefaultTemplateKeyV1::new(owner, 0);
    let parameter = CallableSourceParameterV1::new(
        identifier("value"),
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        CallableParameterCallingV1::Default { template: key },
        origin(),
    );
    let record = CallableSourceInterfaceV1::try_new(
        owner,
        CanonicalCallableSourceParametersV1::try_new(vec![parameter]).unwrap(),
    )
    .unwrap();
    let table = CanonicalCallableSourceInterfacesV1::try_new(vec![record]).unwrap();

    assert!(matches!(
        table.index_templates(&mut NoTemplates),
        Err(CallableSourceInterfaceSetIndexError::Record {
            index: 0,
            error: CallableSourceInterfaceIndexError::Parameter {
                index: 0,
                error: MissingTemplate(missing),
            },
        }) if missing == key
    ));
}

fn empty_record(name: &str) -> CallableSourceInterfaceV1 {
    CallableSourceInterfaceV1::try_new(
        CallableTemplateOrigin::Function(function(name).id()),
        CanonicalCallableSourceParametersV1::try_new(Vec::new()).unwrap(),
    )
    .unwrap()
}

fn ordered(
    left: CallableSourceInterfaceV1,
    right: CallableSourceInterfaceV1,
) -> (CallableSourceInterfaceV1, CallableSourceInterfaceV1) {
    if left.owner() < right.owner() {
        (left, right)
    } else {
        (right, left)
    }
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

fn origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/Defaults.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(0, 1).unwrap(), &context).unwrap(),
    )
}

fn identities<const N: usize>(owners: [CallableTemplateOrigin; N]) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    for owner in owners {
        let CallableTemplateOrigin::Function(id) = owner else {
            panic!("test creates only function source interfaces")
        };
        let record = ["alpha", "omega", "first", "second", "left", "right"]
            .into_iter()
            .map(function)
            .find(|record| record.id() == id)
            .unwrap();
        pending
            .register_external_canonical_authority(record)
            .unwrap();
    }
    pending.finish().unwrap()
}

struct NoTemplates;

impl ExportDefaultTemplateKeyResolver for NoTemplates {
    type Error = MissingTemplate;

    fn resolve_default_template_key(
        &mut self,
        index: u32,
    ) -> Result<ExportDefaultTemplateKeyV1, Self::Error> {
        Err(MissingTemplate(ExportDefaultTemplateKeyV1::new(
            CallableTemplateOrigin::Function(function("missing").id()),
            index,
        )))
    }
}

impl ExportDefaultTemplateIndexResolver for NoTemplates {
    type Error = MissingTemplate;

    fn resolve_default_template_index(
        &mut self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Result<u32, Self::Error> {
        Err(MissingTemplate(key))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MissingTemplate(ExportDefaultTemplateKeyV1);

impl std::fmt::Display for MissingTemplate {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "missing template {:?}", self.0)
    }
}

impl std::error::Error for MissingTemplate {}

struct SourceInterfaceSequence(Vec<CallableSourceInterfaceV1>);

impl WireEncode for SourceInterfaceSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            encoder.map(2)?;
            encoder.field(1)?;
            record.owner().encode(encoder)?;
            encoder.field(2)?;
            encoder.array(0)?;
        }
        Ok(())
    }
}

fn decode_table(value: &impl WireEncode) -> DecodedCanonicalCallableSourceInterfacesV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}
