use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOrigin, DefinitionOriginRecord, DefinitionOriginSubject, DefinitionOwnerAtom,
    DefinitionOwnerChain, DispatchSlotKey, EnumVariantFieldKey, EnumVariantFieldSelector,
    EnumVariantIdentityKey, ExactTypeKey, NonEmptyVec, NormalizedSourcePath, PackagePath,
    PersistentConstructorId, PersistentDispatchSlotId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentFunctionId,
    PersistentGenericFunctionId, PersistentGenericTypeId, PersistentTypeId, SignatureCallableShape,
    SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceNominalKind, SourceSpan,
};

use super::*;
use crate::{CanonicalHirFoundation, CoreProtocolCallableDefinitionV1};

mod builder;
mod operations;
mod surface;

use builder::FixtureBuilder;
use operations::{fixture_operation_owner, operation_entry};
use surface::install_at;

type TypeRecord = CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>;
type GenericTypeRecord = CborIdentityRecord<PersistentGenericTypeId, SourceDeclarationKey>;
type FunctionRecord = CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>;
type GenericFunctionRecord = CborIdentityRecord<PersistentGenericFunctionId, SourceDeclarationKey>;
type ConstructorRecord = CborIdentityRecord<PersistentConstructorId, SourceDeclarationKey>;
type VariantRecord = CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>;
type VariantFieldRecord = CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>;
type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type DispatchRecord = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;

pub(crate) struct ExistingProtocolFixture {
    pub string: TypeRecord,
    pub option: GenericTypeRecord,
    pub option_some: VariantRecord,
    pub option_some_payload: VariantFieldRecord,
    pub option_none: VariantRecord,
    pub exact_types: Vec<ExactTypeRecord>,
}

pub(crate) fn ordinary_origin() -> ConeIdentity {
    scoop_identity::ConeCoordinate::new("test", "protocol-provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap()
}

pub(crate) fn standalone() -> (CoreCompilerProtocolSurfaceV1, CanonicalHirFoundation) {
    standalone_at(ConeIdentity::CORE)
}

pub(crate) fn standalone_at(
    origin: ConeIdentity,
) -> (CoreCompilerProtocolSurfaceV1, CanonicalHirFoundation) {
    let string: TypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(origin, DefinitionOwnerChain::top_level()),
        CanonicalIdentifier::new("String").unwrap(),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap();
    let option: GenericTypeRecord = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(origin, DefinitionOwnerChain::top_level()),
        CanonicalIdentifier::new("Option").unwrap(),
        SourceNominalKind::Enum,
        1,
    ))
    .unwrap();
    let option_some: VariantRecord = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(option.key(), CanonicalIdentifier::new("Some").unwrap())
            .unwrap(),
    )
    .unwrap();
    let option_some_payload: VariantFieldRecord =
        CborIdentityRecord::from_key(EnumVariantFieldKey::new(
            option_some.id(),
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ))
        .unwrap();
    let option_none: VariantRecord = CborIdentityRecord::from_key(
        EnumVariantIdentityKey::source(option.key(), CanonicalIdentifier::new("None").unwrap())
            .unwrap(),
    )
    .unwrap();
    let mut foundation = CanonicalHirFoundation::empty();
    let surface = install_at(
        &mut foundation,
        ExistingProtocolFixture {
            string,
            option,
            option_some,
            option_some_payload,
            option_none,
            exact_types: Vec::new(),
        },
        origin,
    );
    (surface, foundation)
}

pub(crate) fn install(
    foundation: &mut CanonicalHirFoundation,
    existing: ExistingProtocolFixture,
) -> CoreCompilerProtocolSurfaceV1 {
    install_at(foundation, existing, ConeIdentity::CORE)
}

fn product<const N: usize>(entries: [CoreProtocolEntryV1; N]) -> CoreProtocolProductV1<N> {
    CoreProtocolProductV1 { entries }
}

fn concrete_entry(id: PersistentTypeId) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id))
}

fn generic_entry(id: PersistentGenericTypeId) -> CoreProtocolEntryV1 {
    CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id))
}

fn signature_application(
    origin: PersistentGenericTypeId,
    argument: SignatureTypeKey,
) -> SignatureTypeKey {
    SignatureTypeKey::NominalApplication {
        origin,
        arguments: NonEmptyVec::from_first(argument, []),
    }
}

fn generic_protocol_id(entry: &CoreProtocolEntryV1) -> PersistentGenericTypeId {
    match entry {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => *id,
        _ => unreachable!("the fixture protocol nominal is generic"),
    }
}

fn concrete_entry_ref<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id)) => id,
        _ => unreachable!("the fixture fundamental role is a concrete nominal"),
    }
}

fn generic_entry_ref<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentGenericTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("the fixture fundamental role is a generic nominal"),
    }
}

fn site(origin: ConeIdentity, owners: DefinitionOwnerChain) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        origin,
        PackagePath::root(),
        owners,
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn origin_record(origin: ConeIdentity, subject: DefinitionOriginSubject) -> DefinitionOriginRecord {
    let source = SourceIdentity::new(
        origin,
        NormalizedSourcePath::new("src/protocol_fixture.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    DefinitionOriginRecord::new(
        subject,
        DefinitionOrigin::new(source, SourceSpan::new(0, 0).unwrap(), &context).unwrap(),
    )
}
