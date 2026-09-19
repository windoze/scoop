use super::*;

impl Projection<'_, '_> {
    pub(super) fn validate_class(
        &mut self,
        source: &mir::ParamFreeMirTypeExportV1,
        id: mir::ClassId,
        expected: &[mir::MirRepresentationFieldV1],
        kind: Option<mir::MirClassKindV1>,
    ) -> Result<()> {
        let exact = source.exact();
        let definition = arena_get(&self.module.classes, id)
            .ok_or(ExactLayoutLoweringError::SourceRepresentation(exact))?;
        let mir::ClassRepresentation::Declared { fields, base_class } = &definition.representation
        else {
            return Err(ExactLayoutLoweringError::SourceRepresentation(exact));
        };
        let actual_kind = match definition.modifier {
            mir::ClassModifier::Final => mir::MirClassKindV1::Final,
            mir::ClassModifier::Open => mir::MirClassKindV1::Open,
            mir::ClassModifier::Abstract => mir::MirClassKindV1::Abstract,
        };
        if !definition.type_arguments.is_empty() || kind.is_some_and(|kind| kind != actual_kind) {
            return Err(ExactLayoutLoweringError::SourceRepresentation(exact));
        }
        let base_count = match (base_class, source.base_and_interfaces().base) {
            (None, mir::MirBaseClassV1::None) => 0,
            (Some(id), mir::MirBaseClassV1::Base(base))
                if self.exact_of(&mir::Type::Class(*id))? == base =>
            {
                let base = arena_get(&self.module.classes, *id)
                    .ok_or(ExactLayoutLoweringError::SourceBase(exact))?;
                let mir::ClassRepresentation::Declared {
                    fields: base_fields,
                    ..
                } = &base.representation
                else {
                    return Err(ExactLayoutLoweringError::SourceBase(exact));
                };
                if fields.len() < base_fields.len() {
                    return Err(ExactLayoutLoweringError::SourceBase(exact));
                }
                self.meter
                    .charge_work(base_fields.len() as u64, &WirePath::root())?;
                for (field, base) in fields.iter().zip(base_fields) {
                    if self.exact_of(&field.ty)? != self.exact_of(&base.ty)? {
                        return Err(ExactLayoutLoweringError::SourceBase(exact));
                    }
                }
                base_fields.len()
            }
            _ => return Err(ExactLayoutLoweringError::SourceBase(exact)),
        };
        if fields.len() - base_count != expected.len() {
            return Err(ExactLayoutLoweringError::SourceFields(exact));
        }
        self.meter
            .charge_work(expected.len() as u64, &WirePath::root())?;
        for (field, actual) in expected.iter().zip(&fields[base_count..]) {
            if field.value != self.exact_of(&actual.ty)? {
                return Err(ExactLayoutLoweringError::SourceFields(exact));
            }
        }
        Ok(())
    }

    pub(super) fn validate_object(
        &mut self,
        source: &mir::ParamFreeMirTypeExportV1,
        backing: PersistentExactTypeId,
        class: mir::ClassId,
    ) -> Result<()> {
        let record = self.shape(backing)?;
        let mir::MirTypeRepresentationV1::ObjectBacking { declared_fields } =
            record.representation()
        else {
            return Err(ExactLayoutLoweringError::SourceObject(source.exact()));
        };
        self.meter.charge_work(
            source.base_and_interfaces().interfaces.len() as u64,
            &WirePath::root(),
        )?;
        if source.base_and_interfaces() != record.base_and_interfaces() {
            return Err(ExactLayoutLoweringError::SourceObject(source.exact()));
        }
        self.meter
            .charge_work(self.module.objects.len() as u64, &WirePath::root())?;
        let mut objects = self
            .module
            .objects
            .iter()
            .filter(|(_, object)| object.backing_class == class);
        let Some((id, object)) = objects.next() else {
            return Err(ExactLayoutLoweringError::SourceObject(source.exact()));
        };
        if objects.next().is_some() {
            return Err(ExactLayoutLoweringError::SourceObject(source.exact()));
        }
        let object_type = arena_get(&self.module.object_types, object.object_type)
            .ok_or(ExactLayoutLoweringError::SourceObject(source.exact()))?;
        let singleton = arena_get(&self.module.singleton_values, object.singleton_value)
            .ok_or(ExactLayoutLoweringError::SourceObject(source.exact()))?;
        if object_type.declaration != id
            || object_type.representation != class
            || singleton.declaration != id
            || singleton.object_type != object.object_type
        {
            return Err(ExactLayoutLoweringError::SourceObject(source.exact()));
        }
        self.validate_class(record, class, declared_fields, None)
    }
}
