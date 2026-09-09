use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_wire::{
    Decoder, Encoder, RuntimeDecode, RuntimeDecodeError, RuntimeEncode, RuntimeEncodeError,
    WireDecode, WireEncode, WireError, decode_runtime, encode_runtime,
};

use crate::ids::derive_runtime_persistent_id;
use crate::{
    CallableBodyKey, DecodedCallableBodyKey, DecodedPersistentId, PersistentCallableBodyId,
    PersistentId, PersistentIdMismatch,
};

mod derivations;

mod private {
    pub trait CborIdentityKey<I> {}
    pub trait RuntimeIdentityKey<I> {}
}

pub trait CborIdentityKey<I: PersistentId>: WireEncode + private::CborIdentityKey<I> {
    type Error;

    fn derive_identity(&self) -> Result<I, Self::Error>;
}

pub trait RuntimeIdentityKey<I: PersistentId>:
    RuntimeEncode + private::RuntimeIdentityKey<I>
{
    type Error;

    fn derive_identity(&self) -> Result<I, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CborIdentityRecord<I: PersistentId, K> {
    id: I,
    key: K,
}

impl<I: PersistentId, K: CborIdentityKey<I>> CborIdentityRecord<I, K> {
    pub fn from_key(key: K) -> Result<Self, K::Error> {
        let id = key.derive_identity()?;
        Ok(Self { id, key })
    }
}

impl<I: PersistentId, K> CborIdentityRecord<I, K> {
    pub const fn id(&self) -> I {
        self.id
    }

    pub const fn key(&self) -> &K {
        &self.key
    }

    pub fn into_key(self) -> K {
        self.key
    }
}

impl<I: PersistentId + WireEncode, K: WireEncode> WireEncode for CborIdentityRecord<I, K> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.key.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCborIdentityRecord<I: PersistentId, K> {
    id: DecodedPersistentId<I>,
    key: K,
}

impl<I: PersistentId, K> DecodedCborIdentityRecord<I, K> {
    pub const fn decoded_id(&self) -> DecodedPersistentId<I> {
        self.id
    }

    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Resolves an untrusted wire key into its canonical typed key before
    /// recomputing and validating the record identity.
    ///
    /// The resolver is responsible for checking every persistent identity
    /// reference embedded in the decoded key. A concrete [`PersistentId`]
    /// cannot escape through this path unless the rebuilt key derives the
    /// identity carried by the record.
    pub fn resolve<V, E>(
        self,
        resolve_key: impl FnOnce(K) -> Result<V, E>,
    ) -> Result<CborIdentityRecord<I, V>, IdentityRecordResolutionError<E, V::Error, I>>
    where
        V: CborIdentityKey<I>,
    {
        let key = resolve_key(self.key).map_err(IdentityRecordResolutionError::Reference)?;
        let expected = key
            .derive_identity()
            .map_err(IdentityRecordResolutionError::Key)?;
        let id = self
            .id
            .verify(expected)
            .map_err(IdentityRecordResolutionError::Id)?;
        Ok(CborIdentityRecord { id, key })
    }
}

impl<I: PersistentId, K: CborIdentityKey<I>> DecodedCborIdentityRecord<I, K> {
    pub fn validate(
        self,
    ) -> Result<CborIdentityRecord<I, K>, IdentityRecordValidationError<K::Error, I>> {
        let expected = self
            .key
            .derive_identity()
            .map_err(IdentityRecordValidationError::Key)?;
        let id = self
            .id
            .verify(expected)
            .map_err(IdentityRecordValidationError::Id)?;
        Ok(CborIdentityRecord { id, key: self.key })
    }
}

impl<I: PersistentId, K: WireDecode> WireDecode for DecodedCborIdentityRecord<I, K> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let id = decoder.field(1, DecodedPersistentId::<I>::decode)?;
        let key = decoder.field(2, K::decode)?;
        Ok(Self { id, key })
    }
}

impl<I: PersistentId, K: WireEncode> WireEncode for DecodedCborIdentityRecord<I, K> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        self.key.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeIdentityRecord<I> {
    id: I,
    key_bytes: Vec<u8>,
}

impl<I: PersistentId> RuntimeIdentityRecord<I> {
    pub fn from_key<K: RuntimeIdentityKey<I>>(
        key: &K,
    ) -> Result<Self, RuntimeIdentityRecordBuildError<K::Error>> {
        let id = key
            .derive_identity()
            .map_err(RuntimeIdentityRecordBuildError::Key)?;
        let key_bytes = encode_runtime(key).map_err(RuntimeIdentityRecordBuildError::Encode)?;
        Ok(Self { id, key_bytes })
    }

    pub const fn id(&self) -> I {
        self.id
    }
}

impl<I> RuntimeIdentityRecord<I> {
    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }
}

impl<I: WireEncode> WireEncode for RuntimeIdentityRecord<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        encoder.bytes(&self.key_bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedRuntimeIdentityRecord<I: PersistentId> {
    id: DecodedPersistentId<I>,
    key_bytes: Vec<u8>,
}

impl<I: PersistentId> DecodedRuntimeIdentityRecord<I> {
    pub const fn decoded_id(&self) -> DecodedPersistentId<I> {
        self.id
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn decode_key<K: RuntimeDecode>(&self) -> Result<K, RuntimeDecodeError> {
        decode_runtime(&self.key_bytes)
    }

    pub fn validate_key<K>(
        self,
    ) -> Result<(RuntimeIdentityRecord<I>, K), RuntimeIdentityRecordValidationError<K::Error, I>>
    where
        K: RuntimeDecode + RuntimeIdentityKey<I>,
    {
        let key = decode_runtime::<K>(&self.key_bytes)
            .map_err(RuntimeIdentityRecordValidationError::Decode)?;
        let reencoded =
            encode_runtime(&key).map_err(RuntimeIdentityRecordValidationError::Encode)?;
        if reencoded != self.key_bytes {
            return Err(RuntimeIdentityRecordValidationError::NonCanonicalKeyBytes);
        }
        let expected = key
            .derive_identity()
            .map_err(RuntimeIdentityRecordValidationError::Key)?;
        let id = self
            .id
            .verify(expected)
            .map_err(RuntimeIdentityRecordValidationError::Id)?;
        Ok((
            RuntimeIdentityRecord {
                id,
                key_bytes: reencoded,
            },
            key,
        ))
    }
}

impl<I: PersistentId> WireDecode for DecodedRuntimeIdentityRecord<I> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let id = decoder.field(1, DecodedPersistentId::<I>::decode)?;
        let key_bytes = decoder.field(2, Decoder::owned_bytes)?;
        Ok(Self { id, key_bytes })
    }
}

impl<I: PersistentId> WireEncode for DecodedRuntimeIdentityRecord<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encoder.field(2)?;
        encoder.bytes(&self.key_bytes)
    }
}

impl private::RuntimeIdentityKey<PersistentCallableBodyId> for CallableBodyKey {}

impl RuntimeIdentityKey<PersistentCallableBodyId> for CallableBodyKey {
    type Error = scoop_wire::HashError;

    fn derive_identity(&self) -> Result<PersistentCallableBodyId, Self::Error> {
        PersistentCallableBodyId::from_key(self)
    }
}

impl private::RuntimeIdentityKey<PersistentCallableBodyId> for DecodedCallableBodyKey {}

impl RuntimeIdentityKey<PersistentCallableBodyId> for DecodedCallableBodyKey {
    type Error = scoop_wire::HashError;

    fn derive_identity(&self) -> Result<PersistentCallableBodyId, Self::Error> {
        derive_runtime_persistent_id("scoop-callable-body-v1", self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityRecordValidationError<E, I: PersistentId> {
    Key(E),
    Id(PersistentIdMismatch<I>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityRecordResolutionError<R, E, I: PersistentId> {
    Reference(R),
    Key(E),
    Id(PersistentIdMismatch<I>),
}

impl<R: fmt::Display, E: fmt::Display, I: PersistentId> fmt::Display
    for IdentityRecordResolutionError<R, E, I>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Key(error) => error.fmt(formatter),
            Self::Id(error) => error.fmt(formatter),
        }
    }
}

impl<R, E, I> std::error::Error for IdentityRecordResolutionError<R, E, I>
where
    R: std::error::Error + 'static,
    E: std::error::Error + 'static,
    I: PersistentId,
{
}

impl<E: fmt::Display, I: PersistentId> fmt::Display for IdentityRecordValidationError<E, I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(error) => error.fmt(formatter),
            Self::Id(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, I: PersistentId> std::error::Error
    for IdentityRecordValidationError<E, I>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIdentityRecordBuildError<E> {
    Key(E),
    Encode(RuntimeEncodeError),
}

impl<E: fmt::Display> fmt::Display for RuntimeIdentityRecordBuildError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(error) => error.fmt(formatter),
            Self::Encode(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RuntimeIdentityRecordBuildError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeIdentityRecordValidationError<E, I: PersistentId> {
    Decode(RuntimeDecodeError),
    Encode(RuntimeEncodeError),
    NonCanonicalKeyBytes,
    Key(E),
    Id(PersistentIdMismatch<I>),
}

impl<E: fmt::Display, I: PersistentId> fmt::Display for RuntimeIdentityRecordValidationError<E, I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(formatter),
            Self::Encode(error) => error.fmt(formatter),
            Self::NonCanonicalKeyBytes => {
                formatter.write_str("runtime identity key bytes are not canonical")
            }
            Self::Key(error) => error.fmt(formatter),
            Self::Id(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static, I: PersistentId> std::error::Error
    for RuntimeIdentityRecordValidationError<E, I>
{
}

pub fn stable_topological_identity_order<T, I, Id, Dependencies>(
    values: Vec<T>,
    id_of: Id,
    dependencies_of: Dependencies,
) -> Result<Vec<T>, StableIdentityOrderError<I>>
where
    I: PersistentId,
    Id: Fn(&T) -> I,
    Dependencies: Fn(&T) -> Vec<I>,
{
    let mut nodes = BTreeMap::new();
    for value in values {
        let id = id_of(&value);
        if nodes.insert(id, value).is_some() {
            return Err(StableIdentityOrderError::DuplicateIdentity(id));
        }
    }

    let mut indegrees = nodes
        .keys()
        .copied()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<I, Vec<I>>::new();
    for (&id, value) in &nodes {
        let dependencies = dependencies_of(value).into_iter().collect::<BTreeSet<_>>();
        for dependency in dependencies {
            if !nodes.contains_key(&dependency) {
                return Err(StableIdentityOrderError::MissingDependency {
                    identity: id,
                    dependency,
                });
            }
            let Some(indegree) = indegrees.get_mut(&id) else {
                return Err(StableIdentityOrderError::InconsistentGraph(id));
            };
            *indegree += 1;
            dependents.entry(dependency).or_default().push(id);
        }
    }

    let mut ready = indegrees
        .iter()
        .filter_map(|(&id, &indegree)| (indegree == 0).then_some(id))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(nodes.len());
    while let Some(id) = ready.pop_first() {
        let Some(value) = nodes.remove(&id) else {
            return Err(StableIdentityOrderError::InconsistentGraph(id));
        };
        ordered.push(value);
        if let Some(current_dependents) = dependents.get(&id) {
            for dependent in current_dependents {
                let Some(indegree) = indegrees.get_mut(dependent) else {
                    return Err(StableIdentityOrderError::InconsistentGraph(*dependent));
                };
                let Some(next_indegree) = indegree.checked_sub(1) else {
                    return Err(StableIdentityOrderError::InconsistentGraph(*dependent));
                };
                *indegree = next_indegree;
                if next_indegree == 0 {
                    ready.insert(*dependent);
                }
            }
        }
    }

    if let Some((&first, _)) = nodes.first_key_value() {
        return Err(StableIdentityOrderError::Cycle { first });
    }
    Ok(ordered)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StableIdentityOrderError<I: PersistentId> {
    DuplicateIdentity(I),
    MissingDependency { identity: I, dependency: I },
    Cycle { first: I },
    InconsistentGraph(I),
}

impl<I: PersistentId> fmt::Display for StableIdentityOrderError<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity(_) => {
                write!(formatter, "duplicate {} identity", I::KIND)
            }
            Self::MissingDependency { .. } => {
                write!(formatter, "{} identity has a missing dependency", I::KIND)
            }
            Self::Cycle { .. } => write!(formatter, "{} identity table contains a cycle", I::KIND),
            Self::InconsistentGraph(_) => write!(
                formatter,
                "{} identity graph is internally inconsistent",
                I::KIND
            ),
        }
    }
}

impl<I: PersistentId> std::error::Error for StableIdentityOrderError<I> {}

#[cfg(test)]
mod tests {
    use scoop_wire::{
        DecodeLimits, Decoder, Encoder, WireDecode, WireEncode, decode_canonical, encode,
    };

    use super::{
        CborIdentityKey, CborIdentityRecord, DecodedCborIdentityRecord,
        DecodedRuntimeIdentityRecord, IdentityRecordResolutionError, IdentityRecordValidationError,
        RuntimeIdentityRecord, RuntimeIdentityRecordValidationError, StableIdentityOrderError,
        stable_topological_identity_order,
    };
    use crate::ids::derive_persistent_id;
    use crate::{
        CallableBodyKey, ConeIdentity, DecodedCallableBodyKey, DecodedPersistentId, ExactTypeKey,
        PersistentCallableBodyId, PersistentExactTypeId, PersistentFunctionId, PersistentTypeId,
    };

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TestKey(u64);

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct DecodedTestKey(u64);

    impl WireEncode for DecodedTestKey {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.unsigned(self.0)
        }
    }

    impl WireEncode for TestKey {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.unsigned(self.0)
        }
    }

    impl WireDecode for TestKey {
        fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, scoop_wire::WireError> {
            decoder.unsigned().map(Self)
        }
    }

    impl WireDecode for DecodedTestKey {
        fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, scoop_wire::WireError> {
            decoder.unsigned().map(Self)
        }
    }

    impl super::private::CborIdentityKey<PersistentExactTypeId> for TestKey {}

    impl CborIdentityKey<PersistentExactTypeId> for TestKey {
        type Error = scoop_wire::HashError;

        fn derive_identity(&self) -> Result<PersistentExactTypeId, Self::Error> {
            derive_persistent_id("scoop-identity-record-test-v1", self)
        }
    }

    #[test]
    fn cbor_record_decodes_untrusted_id_then_recomputes_it() {
        let record = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(TestKey(7)).unwrap();
        let encoded = encode(&record).unwrap();
        let decoded =
            decode_canonical::<DecodedCborIdentityRecord<PersistentExactTypeId, TestKey>>(
                &encoded,
                DecodeLimits::default(),
            )
            .unwrap();

        assert_eq!(decoded.key(), &TestKey(7));
        assert_eq!(decoded.decoded_id().as_array(), record.id().as_array());
        assert_eq!(decoded.validate().unwrap(), record);

        let mut corrupt = encoded;
        corrupt[4] ^= 1;
        let decoded =
            decode_canonical::<DecodedCborIdentityRecord<PersistentExactTypeId, TestKey>>(
                &corrupt,
                DecodeLimits::default(),
            )
            .unwrap();
        assert!(matches!(
            decoded.validate(),
            Err(IdentityRecordValidationError::Id(_))
        ));
    }

    #[test]
    fn cbor_record_resolves_references_before_recomputing_its_identity() {
        let record = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(TestKey(7)).unwrap();
        let encoded = encode(&record).unwrap();
        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentExactTypeId, DecodedTestKey>,
        >(&encoded, DecodeLimits::default())
        .unwrap();

        let resolved = decoded
            .resolve(|key| Ok::<_, TestReferenceError>(TestKey(key.0)))
            .unwrap();
        assert_eq!(resolved, record);

        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentExactTypeId, DecodedTestKey>,
        >(&encoded, DecodeLimits::default())
        .unwrap();
        assert!(matches!(
            decoded.resolve(|_| Ok::<_, TestReferenceError>(TestKey(8))),
            Err(IdentityRecordResolutionError::Id(_))
        ));

        let decoded = decode_canonical::<
            DecodedCborIdentityRecord<PersistentExactTypeId, DecodedTestKey>,
        >(&encoded, DecodeLimits::default())
        .unwrap();
        assert_eq!(
            decoded.resolve::<TestKey, _>(|_| Err(TestReferenceError)),
            Err(IdentityRecordResolutionError::Reference(TestReferenceError))
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestReferenceError;

    impl std::fmt::Display for TestReferenceError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("test reference was not resolved")
        }
    }

    impl std::error::Error for TestReferenceError {}

    #[test]
    fn cbor_record_registry_uses_the_real_key_derivation() {
        let key = ExactTypeKey::Nominal(PersistentTypeId(ConeIdentity::CORE.0));
        let expected = PersistentExactTypeId::from_key(&key).unwrap();
        let record = CborIdentityRecord::<PersistentExactTypeId, _>::from_key(key).unwrap();

        assert_eq!(record.id(), expected);
    }

    #[test]
    fn runtime_record_decodes_and_reencodes_the_typed_body_key() {
        let key = CallableBodyKey::strong(super::super::StrongCallableDefinitionOwner::Function(
            PersistentFunctionId(ConeIdentity::CORE.0),
        ));
        let record = RuntimeIdentityRecord::<PersistentCallableBodyId>::from_key(&key).unwrap();
        let encoded = encode(&record).unwrap();
        let decoded = decode_canonical::<DecodedRuntimeIdentityRecord<PersistentCallableBodyId>>(
            &encoded,
            DecodeLimits::default(),
        )
        .unwrap();
        let (validated, decoded_key) = decoded.validate_key::<DecodedCallableBodyKey>().unwrap();

        assert_eq!(validated, record);
        assert_eq!(validated.key_bytes(), record.key_bytes());
        assert_eq!(encode(&validated).unwrap(), encoded);
        assert_eq!(
            scoop_wire::encode_runtime(&decoded_key).unwrap(),
            record.key_bytes()
        );

        let malformed = DecodedRuntimeIdentityRecord::<PersistentCallableBodyId> {
            id: DecodedPersistentId::from_unvalidated_bytes(*record.id().as_array()),
            key_bytes: b"\x05\0\0\0".to_vec(),
        };
        assert!(matches!(
            malformed.validate_key::<DecodedCallableBodyKey>(),
            Err(RuntimeIdentityRecordValidationError::Decode(error))
                if error.kind() == (scoop_wire::RuntimeDecodeErrorKind::UnknownTag { tag: 5 })
        ));
    }

    #[test]
    fn stable_order_is_dependency_first_with_raw_id_tie_breaks() {
        #[derive(Debug, Eq, PartialEq)]
        struct Node {
            id: PersistentExactTypeId,
            dependencies: Vec<PersistentExactTypeId>,
        }

        let id = |byte| PersistentExactTypeId([byte; 32]);
        let a = id(1);
        let b = id(2);
        let c = id(3);
        let d = id(4);
        let ordered = stable_topological_identity_order(
            vec![
                Node {
                    id: d,
                    dependencies: vec![c, b],
                },
                Node {
                    id: c,
                    dependencies: vec![a],
                },
                Node {
                    id: b,
                    dependencies: vec![a, a],
                },
                Node {
                    id: a,
                    dependencies: vec![],
                },
            ],
            |node| node.id,
            |node| node.dependencies.clone(),
        )
        .unwrap();

        assert_eq!(
            ordered.into_iter().map(|node| node.id).collect::<Vec<_>>(),
            vec![a, b, c, d]
        );
    }

    #[test]
    fn stable_order_rejects_duplicates_missing_dependencies_and_cycles() {
        #[derive(Clone, Debug, Eq, PartialEq)]
        struct Node(PersistentExactTypeId, Vec<PersistentExactTypeId>);

        let id = |byte| PersistentExactTypeId([byte; 32]);
        let a = id(1);
        let b = id(2);
        assert_eq!(
            stable_topological_identity_order(
                vec![Node(a, vec![]), Node(a, vec![])],
                |node| node.0,
                |node| node.1.clone(),
            ),
            Err(StableIdentityOrderError::DuplicateIdentity(a))
        );
        assert_eq!(
            stable_topological_identity_order(
                vec![Node(a, vec![b])],
                |node| node.0,
                |node| node.1.clone(),
            ),
            Err(StableIdentityOrderError::MissingDependency {
                identity: a,
                dependency: b,
            })
        );
        assert_eq!(
            stable_topological_identity_order(
                vec![Node(b, vec![a]), Node(a, vec![b])],
                |node| node.0,
                |node| node.1.clone(),
            ),
            Err(StableIdentityOrderError::Cycle { first: a })
        );
    }
}
