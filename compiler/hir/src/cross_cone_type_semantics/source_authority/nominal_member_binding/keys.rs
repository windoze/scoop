use super::*;
use scoop_identity::{AccessorRole, PropertyOwner};

pub(super) fn properties<'f>(
    nominals: &BoundNominalSourceContractsV1<'_, 'f>,
    properties: &CanonicalNominalSourcePropertiesV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<PersistentPropertyId, &'f SourceDeclarationKey>, Error> {
    let foundation = nominals.foundation;
    let path = WirePath::root();
    let available = binding_keys::index(
        foundation
            .foundation
            .as_canonical()
            .type_source_property_records(),
        meter,
        &path,
    )?;
    binding_keys::charge_map(properties.records().len(), meter, &path)?;
    let mut keys = BTreeMap::new();
    for record in properties.records() {
        query(available.len(), meter)?;
        let id = record.declaration();
        let key = available
            .get(&id)
            .copied()
            .ok_or(Error::MissingProperty(id))?;
        binding_keys::verify(id, key, foundation.identities, meter, &path)?;
        keys.insert(id, key);
    }
    // Even an omitted setter remains visible in the artifact's own identity
    // table. Const getter identities do not create callable source records.
    let accessors = foundation
        .foundation
        .as_canonical()
        .type_source_accessor_records();
    for record in accessors {
        query(properties.records().len(), meter)?;
        let PropertyOwner::Property(id) = record.key().owner() else {
            continue;
        };
        let Some(property) = properties.get(id) else {
            continue;
        };
        binding_keys::verify(
            record.id(),
            record.key(),
            foundation.identities,
            meter,
            &path,
        )?;
        let expected = match (property.payload(), record.key().role()) {
            (NominalSupportPropertyPayloadV1::Runtime { interface }, AccessorRole::Getter) => {
                Some(interface.getter())
            }
            (NominalSupportPropertyPayloadV1::Runtime { interface }, AccessorRole::Setter) => {
                match interface.mutability() {
                    ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } => Some(*setter),
                    ProtectedPropertyMutabilityV1::ReadOnly => None,
                }
            }
            (NominalSupportPropertyPayloadV1::Const { .. }, AccessorRole::Getter) => {
                Some(record.id())
            }
            (NominalSupportPropertyPayloadV1::Const { .. }, AccessorRole::Setter) => None,
        };
        if expected != Some(record.id()) {
            return Err(Error::Inventory("property accessor keys"));
        }
    }
    Ok(keys)
}

pub(super) fn callables<'f>(
    nominals: &BoundNominalSourceContractsV1<'_, 'f>,
    properties: &BTreeMap<PersistentPropertyId, &'f SourceDeclarationKey>,
    callables: &CanonicalNominalSourceCallablesV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<CallableTemplateOrigin, &'f SourceDeclarationKey>, Error> {
    let foundation = nominals.foundation;
    let canonical = foundation.foundation.as_canonical();
    let path = WirePath::root();
    let functions = binding_keys::index(canonical.type_source_function_records(), meter, &path)?;
    let generics = binding_keys::index(
        canonical.type_source_generic_function_records(),
        meter,
        &path,
    )?;
    binding_keys::charge_map(callables.records().len(), meter, &path)?;
    let mut keys = BTreeMap::new();
    for record in callables.records() {
        let declaration = record.declaration();
        let key = match declaration {
            CallableTemplateOrigin::Function(id) => {
                query(functions.len(), meter)?;
                let key = functions
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingCallableKey(declaration))?;
                binding_keys::verify(id, key, foundation.identities, meter, &path)?;
                key
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                query(generics.len(), meter)?;
                let key = generics
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingCallableKey(declaration))?;
                binding_keys::verify(id, key, foundation.identities, meter, &path)?;
                key
            }
            CallableTemplateOrigin::Accessor(id) => {
                query(
                    foundation.source().entries().accessor_keys.values().len(),
                    meter,
                )?;
                let PropertyOwner::Property(property) = foundation.accessor_key(id)?.owner() else {
                    return Err(Error::MissingCallableKey(declaration));
                };
                query(properties.len(), meter)?;
                properties
                    .get(&property)
                    .copied()
                    .ok_or(Error::MissingProperty(property))?
            }
            CallableTemplateOrigin::VariantConstructor(id) => {
                query(nominals.table().records().len(), meter)?;
                nominals.enum_variant_key(id)?;
                continue;
            }
            CallableTemplateOrigin::Constructor(_) => {
                return Err(Error::MissingCallableKey(declaration));
            }
        };
        keys.insert(declaration, key);
    }
    Ok(keys)
}
