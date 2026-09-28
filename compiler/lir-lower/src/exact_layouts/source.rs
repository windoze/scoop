use scoop_identity::CoreBuiltinNominal;

use super::*;

mod classes;

impl<'a> Projection<'a> {
    pub(super) fn shape(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<&'a mir::ParamFreeMirTypeExportV1> {
        self.types
            .get(exact)
            .ok_or(ExactLayoutLoweringError::MissingMirShape(exact))
    }

    pub(super) fn physical_type(&mut self, exact: PersistentExactTypeId) -> Result<mir::Type> {
        if let Some(record) = self.module.meta.source_exact_types.get_by_identity(exact) {
            let ty = match record.ty() {
                mir::Type::Unit => mir::Type::Unit,
                mir::Type::Any => mir::Type::Any,
                mir::Type::Boolean => mir::Type::Boolean,
                mir::Type::Integer(kind) => mir::Type::Integer(*kind),
                mir::Type::String => mir::Type::String,
                mir::Type::Class(id) => mir::Type::Class(*id),
                mir::Type::Interface(id) => mir::Type::Interface(*id),
                mir::Type::Struct(id) => mir::Type::Struct(*id),
                mir::Type::Enum(id, arguments) => mir::Type::Enum(*id, arguments.clone()),
                _ => return Err(ExactLayoutLoweringError::SourceRepresentation(exact)),
            };
            self.validate_location(&ty, exact)?;
            return Ok(ty);
        }
        if exact
            == PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
                CoreBuiltinNominal::Any.identity_record().id(),
            ))?
        {
            return Ok(mir::Type::Any);
        }

        let ty = match self
            .module
            .meta
            .generated_exact_types
            .get_by_identity(exact)
            .map(|record| record.location())
        {
            Some(mir::GeneratedExactTypeLocation::Class(id)) => Ok(mir::Type::Class(id)),
            Some(mir::GeneratedExactTypeLocation::Enum(id)) => Ok(mir::Type::Enum(id, Vec::new())),
            _ => Err(ExactLayoutLoweringError::MissingPhysicalType(exact)),
        }?;
        self.validate_location(&ty, exact)?;
        Ok(ty)
    }

    fn validate_location(&self, ty: &mir::Type, exact: PersistentExactTypeId) -> Result<()> {
        let valid = match ty {
            mir::Type::Struct(id) => arena_get(&self.module.structs, *id).is_some(),
            mir::Type::Class(id) => arena_get(&self.module.classes, *id).is_some(),
            mir::Type::Interface(id) => arena_get(&self.module.interfaces, *id).is_some(),
            mir::Type::Enum(id, _) => arena_get(&self.module.enums, *id).is_some(),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ExactLayoutLoweringError::SourceRepresentation(exact))
        }
    }

    pub(super) fn exact_of(&mut self, ty: &mir::Type) -> Result<PersistentExactTypeId> {
        if let Some(record) = self.module.meta.source_exact_types.get(ty) {
            let exact = record.identity_record().id();
            self.validate_location(ty, exact)?;
            return Ok(exact);
        }
        if matches!(ty, mir::Type::Any) {
            return Ok(PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
                CoreBuiltinNominal::Any.identity_record().id(),
            ))?);
        }
        let location = match ty {
            mir::Type::Class(id) => mir::GeneratedExactTypeLocation::Class(*id),
            mir::Type::Enum(id, _) => mir::GeneratedExactTypeLocation::Enum(*id),
            _ => return Err(ExactLayoutLoweringError::MissingSourceExact),
        };

        let exact = self
            .module
            .meta
            .generated_exact_types
            .get(location)
            .map(|record| record.exact_record().id())
            .ok_or(ExactLayoutLoweringError::MissingSourceExact)?;
        self.validate_location(ty, exact)?;
        Ok(exact)
    }

    pub(super) fn validate_source(
        &mut self,
        source: &mir::ParamFreeMirTypeExportV1,
        ty: &mir::Type,
    ) -> Result<()> {
        use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Kind};
        let exact = source.exact();

        match (source.representation(), ty) {
            (Kind::InlineArray { element }, mir::Type::Class(id)) => {
                let mir::ClassRepresentation::Intrinsic(
                    mir::IntrinsicTypeRepresentation::Array { element: actual }
                    | mir::IntrinsicTypeRepresentation::MutableArray { element: actual },
                ) = &self.module.classes[*id].representation
                else {
                    return Err(ExactLayoutLoweringError::SourceRepresentation(exact));
                };
                if *element != self.exact_of(actual)? {
                    return Err(ExactLayoutLoweringError::SourceFields(exact));
                }
                Ok(())
            }
            (Kind::Intrinsic(Intrinsic::Unit), mir::Type::Unit)
            | (Kind::Intrinsic(Intrinsic::Boolean), mir::Type::Boolean)
            | (Kind::Intrinsic(Intrinsic::String), mir::Type::String)
            | (Kind::Interface, mir::Type::Interface(_)) => Ok(()),
            (Kind::Intrinsic(Intrinsic::Integer(expected)), mir::Type::Integer(actual))
                if expected == actual =>
            {
                Ok(())
            }
            (
                Kind::Struct {
                    fields,
                    c_layout,
                    interior_mutable,
                },
                mir::Type::Struct(id),
            ) => {
                let definition = &self.module.structs[*id];
                let mir::StructRepresentation::Declared {
                    fields: actual,
                    c_layout: actual_c,
                    interior_mutable: actual_mutable,
                    ..
                } = &definition.representation
                else {
                    return Err(ExactLayoutLoweringError::SourceRepresentation(exact));
                };
                let expected_c = match c_layout {
                    mir::MirTypeCLayoutPolicyV1::Ordinary => None,
                    mir::MirTypeCLayoutPolicyV1::CLayout(contract) => Some(*contract),
                };
                if expected_c != *actual_c
                    || interior_mutable != actual_mutable
                    || definition.gc_free != (source.facts().gc() == mir::MirGcKindV1::GcFree)
                    || fields.len() != actual.len()
                {
                    return Err(ExactLayoutLoweringError::SourceRepresentation(exact));
                }

                for (expected, actual) in fields.iter().zip(actual) {
                    if expected.field != actual.identity
                        || expected.value != self.exact_of(&actual.ty)?
                    {
                        return Err(ExactLayoutLoweringError::SourceFields(exact));
                    }
                }
                Ok(())
            }
            (
                Kind::Enum { variants }
                | Kind::CoroutineStep { variants }
                | Kind::CoroutineSlot { variants },
                mir::Type::Enum(id, _),
            ) => {
                let definition = &self.module.enums[*id];
                if variants.len() != definition.variants.len()
                    || definition.gc_free != (source.facts().gc() == mir::MirGcKindV1::GcFree)
                {
                    return Err(ExactLayoutLoweringError::SourceRepresentation(exact));
                }

                for (variant, actual) in variants.iter().zip(&definition.variants) {
                    if variant.fields.len() != actual.fields.len()
                        || actual.gc_free != (variant.gc == mir::MirGcKindV1::GcFree)
                    {
                        return Err(ExactLayoutLoweringError::SourceFields(exact));
                    }

                    for (field, actual) in variant.fields.iter().zip(&actual.fields) {
                        if field.value != self.exact_of(&actual.ty)? {
                            return Err(ExactLayoutLoweringError::SourceFields(exact));
                        }
                    }
                }
                Ok(())
            }
            (
                Kind::Class {
                    kind,
                    declared_fields,
                },
                mir::Type::Class(id),
            ) => self.validate_class(source, *id, declared_fields, Some(*kind)),
            (
                Kind::Class {
                    declared_fields, ..
                },
                mir::Type::Any,
            ) if declared_fields.is_empty()
                && source.base_and_interfaces().base == mir::MirBaseClassV1::None =>
            {
                Ok(())
            }
            (Kind::Object { backing }, mir::Type::Class(id)) => {
                self.validate_object(source, *backing, *id)
            }
            (Kind::ObjectBacking { declared_fields }, mir::Type::Class(id)) => {
                self.validate_class(source, *id, declared_fields, None)
            }
            (Kind::BoxedValue { payload }, mir::Type::Class(id)) => {
                let boxed = self
                    .module
                    .meta
                    .boxed_types
                    .iter()
                    .find(|boxed| boxed.class() == *id)
                    .ok_or(ExactLayoutLoweringError::SourceRepresentation(exact))?;
                if self.exact_of(boxed.payload())? != payload.value {
                    return Err(ExactLayoutLoweringError::SourceFields(exact));
                }
                self.validate_class(source, *id, std::slice::from_ref(payload), None)
            }
            _ => Err(ExactLayoutLoweringError::SourceRepresentation(exact)),
        }
    }
}

pub(super) fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}
