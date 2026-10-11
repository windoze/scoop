use super::*;

/// Field identities are established once the actual field exists. Constructor
/// lowering can still add delegate storage before this builder is finished.
#[derive(Clone, Debug, Default)]
pub struct HirFieldIdentityBuilder {
    struct_fields: HashMap<StructFieldRef, FieldRecord>,
    class_fields: HashMap<ClassFieldId, FieldRecord>,
    class_by_identity: HashMap<PersistentFieldId, ClassFieldId>,
}

impl HirFieldIdentityBuilder {
    pub fn class_declaration(&self, identity: PersistentFieldId) -> Option<ClassFieldId> {
        self.class_by_identity.get(&identity).copied()
    }

    pub fn struct_field(
        &mut self,
        structs: &Arena<StructDecl>,
        nominal_identities: &HirNominalIdentities,
        field: StructFieldRef,
    ) -> Result<PersistentFieldId, HirFieldIdentityError> {
        if let Some(record) = self.struct_fields.get(&field) {
            return Ok(record.id());
        }
        let record = records::struct_field_record(structs, nominal_identities, field)?;
        let id = record.id();
        self.struct_fields.insert(field, record);
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn class_field(
        &mut self,
        field_id: ClassFieldId,
        field: &ClassField,
        objects: &Arena<ObjectDecl>,
        properties: &Arena<Property>,
        delegate_storages: &Arena<DelegateStorage>,
        nominal_identities: &HirNominalIdentities,
        property_identity: &HirPropertyIdentity,
    ) -> Result<PersistentFieldId, HirFieldIdentityError> {
        if let Some(record) = self.class_fields.get(&field_id) {
            return Ok(record.id());
        }
        let property = match field.source {
            ClassFieldSource::EnclosingInstance => {
                return self.class_enclosing_instance(field_id, field.owner, nominal_identities);
            }
            ClassFieldSource::CapturedEnvironment => {
                return self.class_captured_environment(field_id, field.owner, nominal_identities);
            }
            ClassFieldSource::PrimaryParameter { property, .. }
            | ClassFieldSource::BodyProperty(property) => property,
            ClassFieldSource::InterfaceDelegate { ordinal } => {
                return self.class_interface_delegate(
                    field_id,
                    field.owner,
                    ordinal,
                    nominal_identities,
                );
            }
        };
        let record = records::class_field_record(
            field.owner,
            field_id,
            field,
            property,
            objects,
            properties,
            delegate_storages,
            nominal_identities,
            property_identity,
        )?;
        let id = record.id();
        self.class_fields.insert(field_id, record);
        self.class_by_identity.insert(id, field_id);
        Ok(id)
    }

    pub fn class_interface_delegate(
        &mut self,
        field: ClassFieldId,
        owner: ClassId,
        ordinal: u32,
        nominal_identities: &HirNominalIdentities,
    ) -> Result<PersistentFieldId, HirFieldIdentityError> {
        if let Some(record) = self.class_fields.get(&field) {
            return Ok(record.id());
        }
        let record = records::interface_delegate_record(owner, field, ordinal, nominal_identities)?;
        let id = record.id();
        self.class_fields.insert(field, record);
        self.class_by_identity.insert(id, field);
        Ok(id)
    }

    pub fn class_enclosing_instance(
        &mut self,
        field: ClassFieldId,
        owner: ClassId,
        nominal_identities: &HirNominalIdentities,
    ) -> Result<PersistentFieldId, HirFieldIdentityError> {
        if let Some(record) = self.class_fields.get(&field) {
            return Ok(record.id());
        }
        let record = records::enclosing_instance_record(owner, field, nominal_identities)?;
        let id = record.id();
        self.class_fields.insert(field, record);
        self.class_by_identity.insert(id, field);
        Ok(id)
    }

    pub fn class_captured_environment(
        &mut self,
        field: ClassFieldId,
        owner: ClassId,
        nominal_identities: &HirNominalIdentities,
    ) -> Result<PersistentFieldId, HirFieldIdentityError> {
        if let Some(record) = self.class_fields.get(&field) {
            return Ok(record.id());
        }
        let record = records::captured_environment_record(owner, field, nominal_identities)?;
        let id = record.id();
        self.class_fields.insert(field, record);
        self.class_by_identity.insert(id, field);
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finish(
        mut self,
        structs: &Arena<StructDecl>,
        classes: &Arena<ClassDecl>,
        objects: &Arena<ObjectDecl>,
        class_fields: &Arena<ClassField>,
        properties: &Arena<Property>,
        delegate_storages: &Arena<DelegateStorage>,
        nominal_identities: &HirNominalIdentities,
        property_identities: &HirPropertyIdentities,
    ) -> Result<HirFieldIdentities, HirFieldIdentityError> {
        let mut persistent_ids = HashSet::new();
        let mut struct_by_identity = HashMap::new();
        let mut class_by_identity = self.class_by_identity;
        let mut struct_rows = Vec::with_capacity(structs.len());
        for (struct_id, structure) in structs.iter() {
            let structure_index = raw_index(struct_id);
            let mut records = Vec::with_capacity(structure.semantic_fields().len());
            for field_index in 0..structure.semantic_fields().len() {
                let field_index = u32::try_from(field_index).map_err(|_| {
                    HirFieldIdentityError::TooManyStructFields {
                        structure: structure_index,
                    }
                })?;
                let reference = StructFieldRef::checked(structs, struct_id, field_index)
                    .expect("the field comes from this declaration");
                let record = match self.struct_fields.remove(&reference) {
                    Some(record) => record,
                    None => records::struct_field_record(structs, nominal_identities, reference)?,
                };
                struct_by_identity.insert(record.id(), reference);
                require_unique_id(
                    &mut persistent_ids,
                    record.id(),
                    HirFieldIdentityLocation::Struct {
                        structure: structure_index,
                        field: field_index,
                    },
                )?;
                records.push(record);
            }
            struct_rows.push(records);
        }

        let mut seen_class_fields = vec![false; class_fields.len()];
        let mut class_records = vec![None; class_fields.len()];
        for (class_id, class) in classes.iter() {
            for field_id in &class.fields {
                let field_index = local_index(*field_id);
                if field_index >= class_fields.len() {
                    return Err(HirFieldIdentityError::UnknownClassField {
                        class: raw_index(class_id),
                        field: raw_index(*field_id),
                    });
                }
                if seen_class_fields[field_index] {
                    return Err(HirFieldIdentityError::DuplicateClassField {
                        field: raw_index(*field_id),
                    });
                }
                seen_class_fields[field_index] = true;
                let field = &class_fields[*field_id];
                let record = match self.class_fields.remove(field_id) {
                    Some(record) => record,
                    None => match field.source {
                        ClassFieldSource::EnclosingInstance => {
                            if field.owner != class_id {
                                return Err(HirFieldIdentityError::ClassFieldOwner {
                                    class: raw_index(class_id),
                                    field: raw_index(*field_id),
                                    actual_class: raw_index(field.owner),
                                });
                            }
                            records::enclosing_instance_record(
                                class_id,
                                *field_id,
                                nominal_identities,
                            )?
                        }
                        ClassFieldSource::CapturedEnvironment => {
                            if field.owner != class_id {
                                return Err(HirFieldIdentityError::ClassFieldOwner {
                                    class: raw_index(class_id),
                                    field: raw_index(*field_id),
                                    actual_class: raw_index(field.owner),
                                });
                            }
                            records::captured_environment_record(
                                class_id,
                                *field_id,
                                nominal_identities,
                            )?
                        }
                        ClassFieldSource::PrimaryParameter { property, .. }
                        | ClassFieldSource::BodyProperty(property) => records::class_field_record(
                            class_id,
                            *field_id,
                            field,
                            property,
                            objects,
                            properties,
                            delegate_storages,
                            nominal_identities,
                            &property_identities[property],
                        )?,
                        ClassFieldSource::InterfaceDelegate { ordinal } => {
                            if field.owner != class_id {
                                return Err(HirFieldIdentityError::ClassFieldOwner {
                                    class: raw_index(class_id),
                                    field: raw_index(*field_id),
                                    actual_class: raw_index(field.owner),
                                });
                            }
                            records::interface_delegate_record(
                                class_id,
                                *field_id,
                                ordinal,
                                nominal_identities,
                            )?
                        }
                    },
                };
                class_by_identity.insert(record.id(), *field_id);
                require_unique_id(
                    &mut persistent_ids,
                    record.id(),
                    HirFieldIdentityLocation::ClassField {
                        field: raw_index(*field_id),
                    },
                )?;
                class_records[field_index] = Some(record);
            }
        }
        if let Some(field) = seen_class_fields.iter().position(|seen| !seen) {
            return Err(HirFieldIdentityError::UnownedClassField {
                field: field as u32,
            });
        }
        let class_fields = class_records
            .into_iter()
            .enumerate()
            .map(|(field, record)| {
                record.ok_or(HirFieldIdentityError::UnownedClassField {
                    field: field as u32,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(HirFieldIdentities {
            struct_fields: struct_rows,
            class_fields,
            struct_by_identity,
            class_by_identity,
        })
    }
}
