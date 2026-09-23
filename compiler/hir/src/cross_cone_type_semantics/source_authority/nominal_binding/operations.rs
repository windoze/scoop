//! Declaration-owned value shapes for default-body operation replay.
use super::{BoundNominalSourceContractsV1, NominalSourceBindingError};
use crate::*;
use scoop_identity::{
    PersistentEnumVariantId, PersistentFieldId, PersistentObjectValueId, SignatureTypeKey,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};
mod applied;
mod errors;
mod variants;
pub use errors::DefaultSourceNominalOperationError;
type Error = DefaultSourceNominalOperationError;
pub(in crate::cross_cone_type_semantics::source_authority) use applied::Applied;

/// Nominal value queries only; callable and stored class/property contracts are separate.
#[derive(Clone, Copy, Debug)]
pub enum DefaultSourceNominalOperationV1<'a> {
    Type {
        owner_type: &'a SignatureTypeKey,
        expected: PublicNominalKindV1,
    },
    Struct(&'a SignatureTypeKey),
    Variant(&'a DefaultEnumVariantRefV1),
    VariantField(&'a DefaultEnumVariantFieldRefV1),
    StructField {
        declaration: PersistentFieldId,
        owner_type: &'a SignatureTypeKey,
    },
    Singleton(PersistentObjectValueId),
}

impl BoundNominalSourceContractsV1<'_, '_> {
    /// Replays actual source shape and applies nominal arguments once. The caller
    /// still validates argument bounds, reference access and receiver semantics.
    pub fn default_nominal_operation_shape(
        &self,
        target: DefaultSourceNominalOperationV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<DefaultOperationEntityShapeV1, Error> {
        meter.check_semantic_depth(1, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        use DefaultOperationEntityShapeV1 as Shape;
        use DefaultSourceNominalOperationV1 as Target;
        match target {
            Target::Type {
                owner_type,
                expected,
            } => {
                let applied = Applied::new(self, owner_type, meter, path)?;
                applied.require_kind(expected)?;
                Ok(Shape::Type(applied.owner_type(meter, path)?))
            }
            Target::Struct(owner_type) => {
                let applied = Applied::new(self, owner_type, meter, path)?;
                let fields = applied.struct_shape()?.fields();
                Ok(Shape::Aggregate(applied.aggregate(
                    fields.iter().map(NominalSourceFieldV1::value_type),
                    meter,
                    path,
                )?))
            }
            Target::StructField {
                declaration,
                owner_type,
            } => {
                let applied = Applied::new(self, owner_type, meter, path)?;
                let fields = applied.struct_shape()?.fields();
                let index =
                    self.struct_field_index(applied.source.owner(), declaration, meter, path)?;
                let field = fields.get(index as usize).ok_or_else(|| {
                    NominalSourceBindingError::FieldOwner {
                        owner: applied.source.owner(),
                        field: declaration,
                    }
                })?;
                Ok(Shape::Field(DefaultFieldOperationShapeV1::new(
                    DefaultFieldOperationKindV1::Struct,
                    applied.owner_type(meter, path)?,
                    index,
                    applied.field_type(field.value_type(), meter, path)?,
                    CanonicalBooleanV1::False,
                )))
            }
            Target::Variant(reference) => {
                let applied = Applied::new(self, reference.owner_type(), meter, path)?;
                let variant =
                    self.operation_variant(&applied, reference.declaration(), meter, path)?;
                Ok(Shape::Aggregate(applied.aggregate(
                    variant.fields().iter().map(EnumSourceFieldV1::value_type),
                    meter,
                    path,
                )?))
            }
            Target::VariantField(reference) => self
                .operation_variant_field(reference, meter, path)
                .map(Shape::VariantField),
            Target::Singleton(value) => {
                query(self.objects.len(), meter, path)?;
                self.object_value_key(value)?;
                let subject = self
                    .foundation
                    .default_indirect_access_subject(
                        DefaultSourceIndirectTargetV1::Singleton(value),
                        meter,
                    )
                    .map_err(Error::target)?;
                let scoop_identity::DefinitionOriginSubject::Type(id) = subject else {
                    return Err(Error::Singleton(value));
                };
                query(self.table.records().len(), meter, path)?;
                let source = self.nominal_source(SourceNominalId::Concrete(id))?;
                if !matches!(source.source_shape(), NominalSourceShapeV1::Object(shape) if shape.value() == value)
                {
                    return Err(Error::Singleton(value));
                }
                Ok(Shape::Type(SignatureTypeKey::Nominal(id)))
            }
        }
    }
}
fn query(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.charge_edges(1, path)?;
    meter.charge_work(
        (u64::from(count.max(1).ilog2()) + 1).saturating_mul(32),
        path,
    )
}
