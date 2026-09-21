use super::*;

/// Declaration demand or a structural tuple projection, without an access
/// proof. Tuple shape and position checks belong to operation typing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceFieldAccessSubjectV1 {
    Declaration(Subject),
    TupleElement { declaration_index: u32 },
}

impl BoundTypeFoundationSourcesV1<'_> {
    /// Checks the source owner root of a declared field. A generated object
    /// backing field uses its source object's type, as in sealed HIR signatures.
    pub fn default_field_access_subject(
        &self,
        target: &DefaultFieldRefV1,
        meter: &mut BudgetMeter,
    ) -> Result<DefaultSourceFieldAccessSubjectV1, Error> {
        let mut query = Query {
            foundation: self,
            meter,
            path: WirePath::root(),
        };
        query.meter.check_semantic_depth(1, &query.path)?;
        query.meter.charge_nodes(1, &query.path)?;
        query.meter.charge_work(1, &query.path)?;
        let (target, id, owner_type) = match target {
            DefaultFieldRefV1::Struct {
                declaration,
                owner_type,
            } => (Target::StructField(*declaration), *declaration, owner_type),
            DefaultFieldRefV1::Class {
                declaration,
                owner_type,
            } => (Target::ClassField(*declaration), *declaration, owner_type),
            DefaultFieldRefV1::Tuple { declaration_index } => {
                return Ok(DefaultSourceFieldAccessSubjectV1::TupleElement {
                    declaration_index: *declaration_index,
                });
            }
        };
        let (subject, owner) = query.field(target, id)?;
        query.applied_owner(owner, owner_type)?;
        Ok(DefaultSourceFieldAccessSubjectV1::Declaration(subject))
    }
}
