use super::*;
use scoop_identity::{
    CborIdentityKey, CborIdentityRecord, CoreBuiltinNominal, PersistentId, ValidatedIdentityGraph,
};

#[derive(Debug)]
pub(super) struct Roles {
    pub builtins: [PersistentTypeId; 2],
    pub ptr: PersistentGenericTypeId,
    pub fun_ptr: PersistentGenericTypeId,
}
impl Roles {
    pub fn bind(
        foundation: &ImportedHirFoundation,
        core: &ImportedCoreFundamentalTypeProtocol,
        identities: &ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        if foundation.origin() != ConeIdentity::CORE {
            return Err(Error::CoreRole("provider"));
        }
        let canonical = foundation.canonical_for_semantic_authority();
        let mut builtin = |builtin: CoreBuiltinNominal| {
            meter.charge_owned_bytes(128, &path)?;
            meter.charge_sha256(256, &path)?;
            let record = builtin.identity_record();
            let key = key(
                canonical.type_source_nominal_records(),
                record.id(),
                identities,
                meter,
            )?;
            if key != record.key() {
                return Err(Error::CoreRole("builtin"));
            }
            Ok(record.id())
        };
        let ids = [
            builtin(CoreBuiltinNominal::Unit)?,
            builtin(CoreBuiltinNominal::Any)?,
        ];
        if core.unit().persistent() != ids[0] {
            return Err(Error::CoreRole("Unit"));
        }
        let ptr = core.ptr().persistent();
        let fun_ptr = core.fun_ptr().persistent();
        for (role, id) in [("Ptr", ptr), ("FunPtr", fun_ptr)] {
            let key = key(
                canonical.type_source_generic_records(),
                id,
                identities,
                meter,
            )?;
            if key.origin() != ConeIdentity::CORE
                || key.declaration_kind() != SourceDeclarationKind::Struct
                || key.duplicate_signature().type_parameter_count() != 1
            {
                return Err(Error::CoreRole(role));
            }
        }
        Ok(Self {
            builtins: ids,
            ptr,
            fun_ptr,
        })
    }
}
fn key<'f, I>(
    records: &'f [CborIdentityRecord<I, SourceDeclarationKey>],
    id: I,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<&'f SourceDeclarationKey, Error>
where
    I: PersistentId + 'static,
    SourceDeclarationKey: CborIdentityKey<I>,
{
    let path = WirePath::root();
    meter.check_table_entries(records.len() as u64, &path)?;
    meter.charge_work((records.len() as u64).saturating_mul(65), &path)?;
    meter.charge_work(
        (u64::from(identities.identity_count().max(1).ilog2()) + 1) * 65,
        &path,
    )?;
    let key = records
        .iter()
        .find(|record| record.id() == id)
        .ok_or(Error::CoreRole("artifact key"))?
        .key();
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
    binding_keys::verify(id, key, identities, meter, &path).map_err(Error::foundation)?;
    Ok(key)
}
