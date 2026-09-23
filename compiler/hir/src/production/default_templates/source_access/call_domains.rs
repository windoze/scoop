use super::super::{DefaultEntityProjectionError, arena_get, raw_index};
use super::*;

pub(in crate::production::default_templates) struct SourceCallDomain<'a> {
    pub direct: &'a AccessDomain,
    pub slot: Option<&'a AccessDomain>,
}

impl<'a> SourceCallDomain<'a> {
    pub fn from_owner(
        export: &'a ExportHir,
        owner: ExportParameterOwner,
    ) -> Result<Self, DefaultEntityProjectionError> {
        let missing = |kind, index| DefaultEntityProjectionError::Unknown { kind, index };
        let access = match owner {
            ExportParameterOwner::Function(id) => {
                &arena_get(&export.functions, id)
                    .ok_or_else(|| missing("default source function", raw_index(id)))?
                    .access
            }
            ExportParameterOwner::StructConstructor(id) => {
                &arena_get(&export.struct_constructors, id)
                    .ok_or_else(|| missing("default source struct constructor", raw_index(id)))?
                    .access
            }
            ExportParameterOwner::ClassConstructor(id) => {
                &arena_get(&export.class_constructors, id)
                    .ok_or_else(|| missing("default source class constructor", raw_index(id)))?
                    .access
            }
            ExportParameterOwner::VariantConstructor(variant) => {
                let owner = arena_get(&export.enums, variant.enumeration()).ok_or_else(|| {
                    missing("default source enum", raw_index(variant.enumeration()))
                })?;
                return Ok(Self {
                    direct: &owner.access.lookup.0,
                    slot: None,
                });
            }
        };
        Ok(Self {
            direct: &access.lookup.0,
            slot: access.slot.as_ref().map(|slot| &slot.0),
        })
    }

    pub fn matches(&self, domain: &CallDomain) -> bool {
        self.direct == &domain.direct.0 && self.slot == domain.slot.as_ref().map(|slot| &slot.0)
    }
}
