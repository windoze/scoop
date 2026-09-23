//! Default-body data flow using the already validated declaration closure.

use scoop_hir::{
    CrossConeHirInterfaceSectionV1, DefaultLocalDataFlowSemanticAuthority, ExportDefaultTemplateV1,
    NominalSourceShapeV1, SourceNominalId,
};
use scoop_identity::{PersistentFieldId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WirePath};

use super::CanonicalCrossConeHirSurfaceAuthority;

mod errors;
pub use errors::{CrossConeHirDefaultDataFlowError, CrossConeHirDefaultFieldError};

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(crate) fn validate_default_local_data_flow(
        &mut self,
    ) -> Result<(), CrossConeHirDefaultDataFlowError> {
        let templates = self.current_interface.default_templates();
        if templates.records().is_empty() {
            return Ok(());
        }
        let path = WirePath::root().field(7);
        let interfaces = std::iter::once(self.current_interface)
            .chain(self.dependencies.iter().map(|provider| provider.interface));
        let mut fields = DefaultStructFields::from_interfaces(interfaces, self.meter, &path)?;
        for (index, template) in templates.records().iter().enumerate() {
            template
                .validate_local_data_flow_semantics(&mut fields, self.meter, &path)
                .map_err(|source| CrossConeHirDefaultDataFlowError::Template {
                    index,
                    key: template.key(),
                    source: Box::new(source),
                })?;
        }
        Ok(())
    }
}

struct DefaultStructField {
    declaration: PersistentFieldId,
    owner: SourceNominalId,
    owner_arity: u32,
    index: u32,
}

/// An index over ordinary nominal metadata, never a second declaration table.
/// Building it before visiting bodies keeps every allocation and traversal on
/// the same budget without sharing a mutable meter with the callback.
struct DefaultStructFields(Vec<DefaultStructField>);

impl DefaultStructFields {
    fn from_interfaces<'a, I>(
        interfaces: I,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, CrossConeHirDefaultDataFlowError>
    where
        I: IntoIterator<Item = &'a CrossConeHirInterfaceSectionV1>,
        I::IntoIter: Clone,
    {
        let interfaces = interfaces.into_iter();
        let mut count = 0;
        for interface in interfaces.clone() {
            meter.charge_work(
                interface.nominal_interfaces().declaration_count() as u64 * 2,
                path,
            )?;
            for record in interface.nominal_interfaces().all_records() {
                if let NominalSourceShapeV1::Struct(shape) = record.source_shape() {
                    count += shape.fields().len() as u64;
                    meter.check_table_entries(count, path)?;
                }
            }
        }
        meter.charge_work(count, path)?;
        let mut fields = Vec::new();
        meter.try_reserve_collection_slots(&mut fields, count as usize, path)?;
        for interface in interfaces {
            for record in interface.nominal_interfaces().all_records() {
                let NominalSourceShapeV1::Struct(shape) = record.source_shape() else {
                    continue;
                };
                for (index, field) in shape.fields().iter().enumerate() {
                    fields.push(DefaultStructField {
                        declaration: field.field(),
                        owner: record.declaration(),
                        owner_arity: record.type_parameters().len_u32(),
                        index: index as u32,
                    });
                }
            }
        }
        let levels = u64::from(count.checked_ilog2().unwrap_or(0)) + 1;
        meter.charge_work(count.saturating_mul(levels), path)?;
        fields.sort_unstable_by_key(|field| field.declaration);
        for pair in fields.windows(2) {
            if pair[0].declaration == pair[1].declaration {
                return Err(CrossConeHirDefaultDataFlowError::DuplicateField(
                    pair[0].declaration,
                ));
            }
        }
        Ok(Self(fields))
    }

    fn field_index(
        &self,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
    ) -> Result<u32, CrossConeHirDefaultFieldError> {
        let position = self
            .0
            .binary_search_by_key(&declaration, |field| field.declaration)
            .map_err(|_| CrossConeHirDefaultFieldError::MissingField(declaration))?;
        let field = &self.0[position];
        let (owner, arity) = match owner_type {
            SignatureTypeKey::Nominal(id) => (SourceNominalId::Concrete(*id), 0),
            SignatureTypeKey::NominalApplication { origin, arguments } => (
                SourceNominalId::GenericTemplate(*origin),
                arguments.as_slice().len() as u64,
            ),
            _ => return Err(CrossConeHirDefaultFieldError::NonNominalOwner(declaration)),
        };
        if owner != field.owner {
            return Err(CrossConeHirDefaultFieldError::Owner {
                declaration,
                expected: field.owner,
                actual: owner,
            });
        }
        if arity != u64::from(field.owner_arity) {
            return Err(CrossConeHirDefaultFieldError::Arity {
                declaration,
                expected: field.owner_arity,
                actual: arity,
            });
        }
        Ok(field.index)
    }
}

impl DefaultLocalDataFlowSemanticAuthority<CrossConeHirDefaultFieldError> for DefaultStructFields {
    fn default_binding_struct_field_index(
        &mut self,
        _template: &ExportDefaultTemplateV1,
        declaration: PersistentFieldId,
        owner_type: &SignatureTypeKey,
    ) -> Result<u32, CrossConeHirDefaultFieldError> {
        self.field_index(declaration, owner_type)
    }
}

#[cfg(test)]
mod tests;
