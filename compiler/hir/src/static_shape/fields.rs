use super::*;
use scoop_identity::{PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId};

mod identities;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticFieldIdentity {
    Field(PersistentFieldId),
    VariantField(PersistentEnumVariantFieldId),
}

#[derive(Clone, Copy)]
pub struct StaticFieldShape<'a> {
    pub(super) owner: StaticNominalShape<'a>,
    pub(super) field: &'a Field,
    pub(super) identity: StaticFieldIdentity,
    pub(super) storage: NominalFieldStorage,
    pub(super) annotation: StaticAnnotationTarget,
}

#[derive(Clone, Copy)]
pub(super) enum StaticAnnotationTarget {
    Unannotated,
    Current(SourceAnnotationTarget),
    Dependency(AnnotationTargetV1),
}

impl<'a> StaticFieldShape<'a> {
    pub const fn identity(self) -> StaticFieldIdentity {
        self.identity
    }
    pub fn name(self) -> &'a str {
        &self.field.name
    }
    pub fn value_type(self) -> StaticShapeTypeUse<'a> {
        self.owner.type_use(self.field.ty)
    }
    pub const fn storage(self) -> NominalFieldStorage {
        self.storage
    }
}

#[derive(Clone, Copy)]
pub struct StaticVariantShape<'a> {
    pub(super) owner: StaticNominalShape<'a>,
    pub(super) index: usize,
    pub(super) declaration: &'a Variant,
}

impl<'a> StaticVariantShape<'a> {
    pub fn name(self) -> &'a str {
        &self.declaration.name
    }
    pub const fn style(self) -> VariantStyle {
        self.declaration.style
    }

    pub fn fields(self) -> impl ExactSizeIterator<Item = StaticFieldShape<'a>> {
        self.declaration
            .fields
            .iter()
            .enumerate()
            .map(move |(index, field)| {
                let identity = self.field_identity(index);
                StaticFieldShape {
                    owner: self.owner,
                    field,
                    identity: StaticFieldIdentity::VariantField(identity),
                    storage: NominalFieldStorage::Declared,
                    annotation: self.field_annotation(index, identity),
                }
            })
    }
}

impl<'a> StaticNominalShape<'a> {
    /// Only storage declared by this owner. Enum payloads remain under each
    /// variant, and base fields remain reachable through the base relation.
    pub fn fields(self) -> impl ExactSizeIterator<Item = StaticFieldShape<'a>> {
        let fields = match self.definition {
            nominals::Definition::Struct(value) => value.semantic_fields(),
            nominals::Definition::Class(value) => value.fields.as_slice(),
            _ => &[],
        };
        fields.iter().enumerate().map(move |(index, field)| {
            let (identity, storage, annotation) = self.field_identity(index);
            StaticFieldShape {
                owner: self,
                field,
                identity: StaticFieldIdentity::Field(identity),
                storage,
                annotation,
            }
        })
    }

    pub fn variants(self) -> impl ExactSizeIterator<Item = StaticVariantShape<'a>> {
        let variants = match self.definition {
            nominals::Definition::Enum(value) => value.variants.as_slice(),
            _ => &[],
        };
        variants
            .iter()
            .enumerate()
            .map(move |(index, declaration)| StaticVariantShape {
                owner: self,
                index,
                declaration,
            })
    }
}
