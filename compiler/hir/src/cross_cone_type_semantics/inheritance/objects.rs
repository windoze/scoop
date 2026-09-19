use scoop_identity::{
    ExactTypeKey, GeneratedNominalKey, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKind,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    CheckedNominalInheritanceGraphV1, InheritanceGraphError, InheritanceQueryError,
    NominalInheritanceSemanticAuthority,
};
use crate::{NominalRepresentationShapeV1, SourceNominalId};

/// The semantic source object and its physical generated class remain two
/// different exact identities, even when they admit the same receiver value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedObjectInheritanceRelationV1 {
    source_object: PersistentExactTypeId,
    backing_class: PersistentExactTypeId,
}
impl CheckedObjectInheritanceRelationV1 {
    pub const fn source_object(&self) -> PersistentExactTypeId {
        self.source_object
    }
    pub const fn backing_class(&self) -> PersistentExactTypeId {
        self.backing_class
    }
}

impl CheckedNominalInheritanceGraphV1<'_> {
    pub fn object_backing_relation(
        &self,
        object: PersistentExactTypeId,
    ) -> Option<CheckedObjectInheritanceRelationV1> {
        self.object_backings.get(&object).copied()
    }

    pub(super) fn validate_object<A: NominalInheritanceSemanticAuthority<E>, E>(
        &mut self,
        exact: PersistentExactTypeId,
        object: PersistentTypeId,
        authority: &A,
        meter: &mut BudgetMeter,
    ) -> Result<(), InheritanceGraphError<E>> {
        let path = WirePath::root();
        meter
            .charge_work(3, &path)
            .map_err(InheritanceGraphError::Resource)?;
        let representation = authority
            .object_representation(object)
            .map_err(InheritanceGraphError::Foundation)?;
        let source = self.sources[&SourceNominalId::Concrete(object)];
        if representation.owner() != object || representation.declaration_access() != source.access
        {
            return Err(InheritanceGraphError::ObjectBacking(exact));
        }
        let NominalRepresentationShapeV1::Object { backing_class, .. } = representation.shape()
        else {
            return Err(InheritanceGraphError::ObjectBacking(exact));
        };
        let generated = authority
            .generated_nominal_key(*backing_class)
            .map_err(InheritanceGraphError::Foundation)?;
        if generated != &(GeneratedNominalKey::ObjectBackingClass { object })
            || PersistentTypeId::from_generated_key(generated).ok() != Some(*backing_class)
        {
            return Err(InheritanceGraphError::ObjectBacking(exact));
        }
        let expected = ExactTypeKey::Nominal(*backing_class);
        let backing_exact = PersistentExactTypeId::from_key(&expected)
            .map_err(|_| InheritanceGraphError::ObjectBacking(exact))?;
        if authority
            .exact_type_key(backing_exact)
            .map_err(InheritanceGraphError::Foundation)?
            != &expected
        {
            return Err(InheritanceGraphError::ObjectBacking(exact));
        }
        meter
            .charge_nodes(1, &path)
            .map_err(InheritanceGraphError::Resource)?;
        meter
            .charge_collection_slots(1, &path)
            .map_err(InheritanceGraphError::Resource)?;
        self.object_backings.insert(
            exact,
            CheckedObjectInheritanceRelationV1 {
                source_object: exact,
                backing_class: backing_exact,
            },
        );
        Ok(())
    }

    pub(in crate::cross_cone_type_semantics) fn access_class_exact(
        &self,
        source: PersistentExactTypeId,
    ) -> Result<PersistentExactTypeId, InheritanceQueryError> {
        if let Some(relation) = self.object_backings.get(&source) {
            return Ok(relation.backing_class);
        }
        self.require_class(source)?;
        Ok(source)
    }

    pub(in crate::cross_cone_type_semantics) fn receiver_is_access_subtype(
        &self,
        receiver: PersistentExactTypeId,
        access_subject: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<bool, InheritanceQueryError> {
        if self.object_backings.contains_key(&access_subject) {
            // A source object is final. Its physical backing-class id is not a
            // source static receiver type and cannot be substituted here.
            meter
                .charge_work(1, &WirePath::root())
                .map_err(InheritanceQueryError::Resource)?;
            self.nodes
                .get(&receiver)
                .ok_or(InheritanceQueryError::UnknownExact(receiver))?;
            return Ok(receiver == access_subject);
        }
        self.is_subclass(receiver, access_subject, meter)
    }

    pub(in crate::cross_cone_type_semantics) fn is_class_access_scope(
        &self,
        source: SourceNominalId,
    ) -> Result<bool, InheritanceQueryError> {
        let declaration = self
            .sources
            .get(&source)
            .ok_or(InheritanceQueryError::UnknownSource(source))?;
        Ok(matches!(
            declaration.key.declaration_kind(),
            SourceDeclarationKind::Class | SourceDeclarationKind::Object
        ))
    }
}
