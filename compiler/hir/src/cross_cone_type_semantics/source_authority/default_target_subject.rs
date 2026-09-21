//! Artifact-owned identity routes to declarations governing indirect accesses.
use super::binding_keys;
use crate::*;
use scoop_identity::{DefinitionOriginSubject as Subject, *};
use scoop_wire::{BudgetMeter, WireError, WirePath};
mod applied_fields;
mod callables;
mod constructors;
mod errors;
mod fields;
mod globals;
mod keys;
mod owners;
pub use applied_fields::DefaultSourceFieldAccessSubjectV1;
pub use callables::DefaultSourceCallableAccessSubjectV1;
pub use errors::DefaultSourceTargetSubjectError;
type Error = DefaultSourceTargetSubjectError;

/// Non-declaration targets whose visibility belongs to an actual source
/// declaration. This enum is a query input, not a wire format or access proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceIndirectTargetV1 {
    StructField(PersistentFieldId),
    ClassField(PersistentFieldId),
    EnumVariant(PersistentEnumVariantId),
    Singleton(PersistentObjectValueId),
}
type Target = DefaultSourceIndirectTargetV1;

impl BoundTypeFoundationSourcesV1<'_> {
    /// Derives a declaration demand from actual local artifact keys. Applied
    /// owner types, declaration access and runtime capabilities remain separate.
    pub fn default_indirect_access_subject(
        &self,
        target: Target,
        meter: &mut BudgetMeter,
    ) -> Result<Subject, Error> {
        let mut query = Query {
            foundation: self,
            meter,
            path: WirePath::root(),
        };
        query.meter.check_semantic_depth(1, &query.path)?;
        query.meter.charge_nodes(1, &query.path)?;
        let canonical = self.foundation.as_canonical();
        match target {
            Target::StructField(id) | Target::ClassField(id) => {
                query.field(target, id).map(|(subject, _)| subject)
            }
            Target::EnumVariant(id) => {
                let key = query.key(canonical.type_source_enum_variant_records(), id, || {
                    Error::MissingTarget(target)
                })?;
                let owner = key.source_owner().ok_or(Error::Role(target))?;
                query.nominal(owner, SourceDeclarationKind::Enum)
            }
            Target::Singleton(id) => {
                let key = query.key(canonical.type_source_object_value_records(), id, || {
                    Error::MissingTarget(target)
                })?;
                NominalRepresentationSupportV1::charge_source_key_resources(
                    key,
                    query.meter,
                    &query.path,
                )?;
                let bytes = scoop_wire::encoded_length(key).map_err(Error::Encoding)?;
                query.meter.charge_sha256(bytes, &query.path)?;
                let owner =
                    SourceNominalId::from_source_declaration(key).map_err(Error::Identity)?;
                query.nominal(owner, SourceDeclarationKind::Object)
            }
        }
    }
}

struct Query<'b, 'f, 'm> {
    foundation: &'b BoundTypeFoundationSourcesV1<'f>,
    meter: &'m mut BudgetMeter,
    path: WirePath,
}
fn subject(owner: SourceNominalId) -> Subject {
    match owner {
        SourceNominalId::Concrete(id) => Subject::Type(id),
        SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
    }
}
