use super::*;
use scoop_identity::{IdentityReferenceError, SourceDeclarationKey};

mod ownership;
pub(in crate::cross_cone_type_bridge) mod roots;

#[derive(Debug)]
pub enum MirTypeBridgeSourceJoinError<E> {
    Source(E),
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Reference(IdentityReferenceError),
    Shape(MirShapeSupportError),
    GeneratedIdentity(scoop_identity::GeneratedNominalIdentityError),
    ExactIdentity(scoop_wire::HashError),
    Inventory(MirTypeBridgeSourceInventoryV1),
    NonCanonicalInventory(MirTypeBridgeSourceInventoryV1),
    Record(MirTypeBridgeSourceRecordV1),
    Ownership(MirTypeBridgeSourceRecordV1),
    SourceRootProvider { source: PersistentTypeId },
    SourceRootType { source: PersistentTypeId },
    Provider,
    WorkOverflow,
}
impl<E: std::fmt::Debug> std::fmt::Display for MirTypeBridgeSourceJoinError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR source bridge: {self:?}")
    }
}
impl<E: std::fmt::Debug> std::error::Error for MirTypeBridgeSourceJoinError<E> {}

impl MirTypeBridgeExportConstituentsV1 {
    pub fn validate_sources<'a, A: MirTypeBridgeSourceSemanticAuthorityV1<E>, E>(
        &'a self,
        provider: ConeIdentity,
        identities: &ValidatedIdentityGraph,
        authority: &A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedMirTypeBridgeSourceJoinV1<'a>, MirTypeBridgeSourceJoinError<E>> {
        use MirTypeBridgeSourceInventoryV1 as Inventory;
        use MirTypeBridgeSourceJoinError as Error;
        use MirTypeBridgeSourceRecordV1 as Record;
        if authority.provider() != provider || self.shapes.provider() != provider {
            return Err(Error::Provider);
        }
        inventory(
            self.types.records().iter().map(|r| r.exact()),
            authority.required_types().map_err(Error::Source)?,
            Inventory::Types,
            meter,
        )?;
        inventory(
            self.callables.entries().iter().map(|r| r.implementation()),
            authority.required_callables().map_err(Error::Source)?,
            Inventory::Callables,
            meter,
        )?;
        inventory(
            self.dispatch.records().iter().map(|r| r.owner()),
            authority.required_dispatch().map_err(Error::Source)?,
            Inventory::Dispatch,
            meter,
        )?;
        inventory(
            self.objects.records().iter().map(|r| r.value()),
            authority.required_objects().map_err(Error::Source)?,
            Inventory::Objects,
            meter,
        )?;
        for record in self.types.records() {
            ownership::type_record(record, provider, identities, meter)?;
            compare(
                record,
                authority
                    .type_source(record.exact())
                    .map_err(Error::Source)?,
                Record::Type(record.exact()),
                meter,
            )?;
        }
        for record in self.callables.entries() {
            ownership::callable(record, provider, identities, meter)?;
            compare(
                record,
                authority
                    .callable_source(record.implementation())
                    .map_err(Error::Source)?,
                Record::Callable(record.implementation()),
                meter,
            )?;
        }
        for record in self.dispatch.records() {
            if self.types.get(record.owner()).is_none() {
                return Err(Error::Ownership(Record::Dispatch(record.owner())));
            }
            compare(
                record,
                authority
                    .dispatch_source(record.owner())
                    .map_err(Error::Source)?,
                Record::Dispatch(record.owner()),
                meter,
            )?;
        }
        for record in self.objects.records() {
            if record.provider() != provider {
                return Err(Error::Provider);
            }
            compare(
                record,
                authority
                    .object_source(record.value())
                    .map_err(Error::Source)?,
                Record::Object(record.value()),
                meter,
            )?;
        }
        roots::validate(
            self,
            provider,
            identities,
            authority.required_source_roots().map_err(Error::Source)?,
            meter,
        )?;
        compare(
            &self.initialization_uses,
            authority
                .committed_initialization_uses()
                .map_err(Error::Source)?,
            Record::InitializationUses,
            meter,
        )?;
        Ok(CheckedMirTypeBridgeSourceJoinV1 {
            provider,
            exports: self,
        })
    }
}

fn inventory<T: Copy + Ord, E>(
    actual: impl ExactSizeIterator<Item = T>,
    expected: &[T],
    kind: MirTypeBridgeSourceInventoryV1,
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    meter
        .check_table_entries(expected.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    meter
        .charge_work(actual.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    meter
        .charge_work(expected.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    if expected.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::NonCanonicalInventory(kind));
    }
    if !actual.eq(expected.iter().copied()) {
        return Err(Error::Inventory(kind));
    }
    Ok(())
}

fn compare<T: WireEncode + PartialEq, E>(
    actual: &T,
    expected: &T,
    record: MirTypeBridgeSourceRecordV1,
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let work = scoop_wire::encoded_length(actual)
        .map_err(Error::Encoding)?
        .checked_add(scoop_wire::encoded_length(expected).map_err(Error::Encoding)?)
        .ok_or(Error::WorkOverflow)?;
    meter
        .charge_work(work, &WirePath::root())
        .map_err(Error::Resource)?;
    if actual != expected {
        return Err(Error::Record(record));
    }
    Ok(())
}
