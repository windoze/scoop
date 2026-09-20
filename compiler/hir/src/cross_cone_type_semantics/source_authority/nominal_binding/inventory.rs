use super::*;
use scoop_identity::{
    CborIdentityKey, CborIdentityRecord, DefinitionOwnerAtom, PersistentConstructorId,
    PersistentFunctionId, PersistentGenericFunctionId, PersistentId, PersistentPropertyId,
};

#[derive(Default)]
struct Inventory {
    constructors: BTreeSet<PersistentConstructorId>,
    functions: BTreeSet<PersistentFunctionId>,
    generic_functions: BTreeSet<PersistentGenericFunctionId>,
    properties: BTreeSet<PersistentPropertyId>,
    children: BTreeSet<SourceNominalId>,
}

pub(super) fn validate(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    table: &CanonicalNominalSourceContractsV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    binding_keys::charge_map(table.records().len(), meter, &path)?;
    let mut inventories: BTreeMap<_, _> = table
        .records()
        .iter()
        .map(|record| (record.owner(), Inventory::default()))
        .collect();
    let own = foundation.foundation.as_canonical();
    macro_rules! scan {
        ($records:expr, $include:expr, $insert:expr) => {
            scan(
                foundation,
                table,
                &mut inventories,
                $records,
                $include,
                $insert,
                meter,
            )?;
        };
    }
    scan!(
        own.type_source_constructor_records(),
        |kind| matches!(
            kind,
            PublicNominalKindV1::Class | PublicNominalKindV1::Struct
        ),
        |i: &mut Inventory, id| {
            i.constructors.insert(id);
        }
    );
    scan!(
        own.type_source_function_records(),
        |_| true,
        |i: &mut Inventory, id| {
            i.functions.insert(id);
        }
    );
    scan!(
        own.type_source_generic_function_records(),
        |_| true,
        |i: &mut Inventory, id| {
            i.generic_functions.insert(id);
        }
    );
    scan!(
        own.type_source_property_records(),
        |_| true,
        |i: &mut Inventory, id| {
            i.properties.insert(id);
        }
    );
    scan!(
        own.type_source_nominal_records(),
        |_| true,
        |i: &mut Inventory, id| {
            i.children.insert(SourceNominalId::Concrete(id));
        }
    );
    scan!(
        own.type_source_generic_records(),
        |_| true,
        |i: &mut Inventory, id| {
            i.children.insert(SourceNominalId::GenericTemplate(id));
        }
    );
    for record in table.records() {
        let expected = &inventories[&record.owner()];
        let members = record.members().values();
        let count = members
            .len()
            .saturating_add(record.constructors().values().len())
            .saturating_add(record.children().values().len());
        meter.check_table_entries(count as u64, &path)?;
        meter.charge_work((count as u64).saturating_mul(3), &path)?;
        if !expected
            .constructors
            .iter()
            .eq(record.constructors().values())
        {
            return Err(Error::Inventory("nominal constructors"));
        }
        if !expected
            .functions
            .iter()
            .copied()
            .eq(members.iter().filter_map(|member| match member {
                NestedSourceMemberRefV1::Function(id) => Some(*id),
                _ => None,
            }))
            || !expected
                .generic_functions
                .iter()
                .copied()
                .eq(members.iter().filter_map(|member| match member {
                    NestedSourceMemberRefV1::GenericFunction(id) => Some(*id),
                    _ => None,
                }))
            || !expected
                .properties
                .iter()
                .copied()
                .eq(members.iter().filter_map(|member| match member {
                    NestedSourceMemberRefV1::Property(id) => Some(*id),
                    _ => None,
                }))
        {
            return Err(Error::Inventory("nominal members"));
        }
        if !expected.children.iter().eq(record.children().values()) {
            return Err(Error::Inventory("nominal children"));
        }
    }
    Ok(())
}

fn scan<I>(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    table: &CanonicalNominalSourceContractsV1,
    inventories: &mut BTreeMap<SourceNominalId, Inventory>,
    records: &[CborIdentityRecord<I, SourceDeclarationKey>],
    include: impl Fn(PublicNominalKindV1) -> bool,
    mut insert: impl FnMut(&mut Inventory, I),
    meter: &mut BudgetMeter,
) -> Result<(), Error>
where
    I: PersistentId + 'static,
    SourceDeclarationKey: CborIdentityKey<I>,
{
    let path = WirePath::root();
    // At most one inventory entry is allocated per scanned artifact record.
    binding_keys::charge_map(records.len(), meter, &path)?;
    for record in records {
        queries(inventories.len(), meter)?;
        let key = record.key();
        let Some(owner) = direct_owner(key) else {
            continue;
        };
        let Some(source) = table.get(owner) else {
            continue;
        };
        if !include(source.kind()) {
            continue;
        }
        binding_keys::verify(record.id(), key, foundation.identities, meter, &path)?;
        NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &path)?;
        let parent = foundation.nominal_key(owner)?;
        let chain = key.owners().owners();
        if key.origin() != foundation.source().entries().provider
            || chain[..chain.len() - 1] != *parent.owners().owners()
            || key.package() != parent.package()
        {
            return Err(invalid(
                owner,
                "declaration identity has a different lexical source owner",
            ));
        }
        // The table and this map were built from the same exact root set.
        let inventory = inventories
            .get_mut(&owner)
            .ok_or(Error::MissingSource(owner))?;
        insert(inventory, record.id());
    }
    Ok(())
}

fn direct_owner(key: &SourceDeclarationKey) -> Option<SourceNominalId> {
    match key.owners().owners().last()? {
        DefinitionOwnerAtom::Type(id) => Some(SourceNominalId::Concrete(*id)),
        DefinitionOwnerAtom::GenericType(id) => Some(SourceNominalId::GenericTemplate(*id)),
        _ => None,
    }
}
