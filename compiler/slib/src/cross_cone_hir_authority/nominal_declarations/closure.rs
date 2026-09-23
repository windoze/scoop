use super::*;
use scoop_hir::{NominalSourceShapeV1, SignatureNominalWalker};
use std::collections::BTreeSet;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_shared_nominal_inventory(&mut self) -> Result<(), Error> {
        let interface = self.current_interface;
        let mut closure = Closure {
            world: self,
            required: BTreeSet::new(),
            pending: Vec::new(),
        };
        for record in interface.nominal_interfaces().records() {
            closure.add(record.declaration())?;
        }
        while let Some(owner) = closure.pending.pop() {
            let record = interface
                .nominal_interfaces()
                .declaration(owner)
                .ok_or_else(|| invalid(owner, "required source declaration is absent"))?;
            let key = closure.world.source_nominal_key(owner)?;
            if let Some(atom) = key.owners().owners().last() {
                let parent = nominal_atom(atom)
                    .ok_or_else(|| invalid(owner, "invalid source lexical parent"))?;
                closure.add(parent)?;
            }
            for ty in record.exact_supertypes().values() {
                closure.signature(ty)?;
            }
            for field in record.source_shape().declared_fields() {
                closure.signature(field.value_type())?;
            }
            if let NominalSourceShapeV1::Enum(shape) = record.source_shape() {
                for variant in shape.variants() {
                    for field in variant.fields() {
                        closure.signature(field.value_type())?;
                    }
                }
            }
            for child in record.declaration_details().children().values() {
                closure.add(*child)?;
            }
        }
        for record in interface.nominal_interfaces().support_records() {
            closure.world.charge_declaration_work(1)?;
            if !closure.required.contains(&record.declaration()) {
                return Err(invalid(
                    record.declaration(),
                    "unrelated source support declaration",
                ));
            }
        }
        Ok(())
    }
}

struct Closure<'a, 'b> {
    world: &'a mut CanonicalCrossConeHirSurfaceAuthority<'b>,
    required: BTreeSet<SourceNominalId>,
    pending: Vec<SourceNominalId>,
}

impl Closure<'_, '_> {
    fn add(&mut self, owner: SourceNominalId) -> Result<(), Error> {
        self.world.charge_declaration_work(1)?;
        if let SourceNominalId::Concrete(id) = owner
            && [
                scoop_identity::CoreBuiltinNominal::Unit,
                scoop_identity::CoreBuiltinNominal::Any,
            ]
            .iter()
            .any(|builtin| builtin.identity_record().id() == id)
        {
            return Ok(());
        }
        let key = self.world.source_nominal_key(owner)?;
        if key.origin() != self.world.current {
            self.world.provider_interface(key.origin())?;
            return Ok(());
        }
        if self.required.contains(&owner) {
            return Ok(());
        }
        if self
            .world
            .current_interface
            .nominal_interfaces()
            .declaration(owner)
            .is_none()
        {
            return Err(invalid(
                owner,
                "required source support declaration is absent",
            ));
        }
        let path = WirePath::root();
        self.world
            .meter
            .check_table_entries(self.required.len() as u64 + 1, &path)
            .map_err(Error::Resource)?;
        self.world
            .meter
            .charge_collection_slots(1, &path)
            .map_err(Error::Resource)?;
        self.world
            .meter
            .try_reserve_collection_slots(&mut self.pending, 1, &path)
            .map_err(Error::Resource)?;
        self.required.insert(owner);
        self.pending.push(owner);
        Ok(())
    }

    fn signature(&mut self, signature: &SignatureTypeKey) -> Result<(), Error> {
        let path = WirePath::root();
        let mut walk = SignatureNominalWalker::new(signature, self.world.meter, &path)
            .map_err(Error::Resource)?;
        while let Some(owner) = walk
            .next(self.world.meter, &path)
            .map_err(Error::Resource)?
        {
            self.add(owner)?;
        }
        Ok(())
    }
}
