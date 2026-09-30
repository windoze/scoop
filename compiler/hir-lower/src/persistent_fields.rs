use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

pub(crate) fn build(
    lowerer: &Lowerer,
    nominal_identities: &hir::HirNominalIdentities,
    property_identities: &hir::HirPropertyIdentities,
    builder: hir::HirFieldIdentityBuilder,
) -> Result<hir::HirFieldIdentities, PersistentFieldIdentityError> {
    builder
        .finish(
            &lowerer.structs,
            &lowerer.classes,
            &lowerer.objects,
            &lowerer.class_fields,
            &lowerer.properties,
            &lowerer.delegate_storages,
            nominal_identities,
            property_identities,
        )
        .map_err(|error| {
            let location = error
                .structure()
                .and_then(|raw| {
                    let structure = hir::StructId::from_raw(raw.into());
                    ((raw as usize) < lowerer.structs.len()).then(|| {
                        (
                            lowerer.struct_files.get(&structure).copied().unwrap_or(0),
                            lowerer.structs[structure].span,
                        )
                    })
                })
                .or_else(|| {
                    error.class().and_then(|raw| {
                        let class = hir::ClassId::from_raw(raw.into());
                        ((raw as usize) < lowerer.classes.len()).then(|| {
                            (
                                lowerer.class_files.get(&class).copied().unwrap_or(0),
                                lowerer.classes[class].span,
                            )
                        })
                    })
                })
                .or_else(|| {
                    error.class_field().and_then(|raw| {
                        let field = hir::ClassFieldId::from_raw(raw.into());
                        ((raw as usize) < lowerer.class_fields.len()).then(|| {
                            let field = &lowerer.class_fields[field];
                            (
                                lowerer.class_files.get(&field.owner).copied().unwrap_or(0),
                                field.span,
                            )
                        })
                    })
                });
            let (file, span) = location.unwrap_or((0, Span { start: 0, end: 0 }));
            PersistentFieldIdentityError { file, span, error }
        })
}

#[derive(Debug)]
pub(crate) struct PersistentFieldIdentityError {
    file: usize,
    span: Span,
    error: hir::HirFieldIdentityError,
}

impl PersistentFieldIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentFieldIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent field identity: {}",
            self.error
        )
    }
}

impl std::error::Error for PersistentFieldIdentityError {}

#[cfg(test)]
mod tests;

impl Lowerer {
    pub(crate) fn struct_field_reference(
        &mut self,
        application: hir::StructApplicationId,
        index: u32,
    ) -> hir::FieldRef {
        let application = &self.struct_applications[application];
        let owner = application.canonical_type;
        if let Some(definition) = self.loaded_struct_definitions.get(&application.template) {
            return hir::FieldRef::StructField {
                owner,
                field: definition.field_identity(index as usize),
            };
        }
        let reference = hir::StructFieldRef::checked(
            &self.structs,
            self.struct_id(application.template),
            index,
        )
        .expect("the field belongs to its declaring struct");
        let field = self
            .field_identity_builder
            .struct_field(
                &self.structs,
                self.nominal_identities
                    .as_ref()
                    .expect("nominal identities precede fields"),
                reference,
            )
            .expect("a resolved struct field has its original declaration identity");
        hir::FieldRef::StructField { owner, field }
    }

    pub(crate) fn class_field_identity(
        &mut self,
        field: hir::ClassFieldId,
    ) -> scoop_identity::PersistentFieldId {
        let property = self.ordinary_property_identity(self.class_fields[field].property);
        self.field_identity_builder
            .class_field(
                field,
                &self.class_fields[field],
                &self.objects,
                &self.properties,
                &self.delegate_storages,
                self.nominal_identities
                    .as_ref()
                    .expect("nominal identities precede fields"),
                &property,
            )
            .expect("a resolved class field has its original storage identity")
    }

    pub(crate) fn class_field_reference(
        &mut self,
        application: hir::ClassApplicationId,
        field: hir::ClassFieldId,
    ) -> hir::FieldRef {
        hir::FieldRef::ClassField {
            owner: self.class_applications[application].canonical_type,
            field: self.class_field_identity(field),
        }
    }

    pub(crate) fn initializing_class_field_reference(
        &mut self,
        application: hir::ClassApplicationId,
        field: hir::ClassFieldId,
    ) -> hir::InitializingClassFieldRef {
        hir::InitializingClassFieldRef {
            owner: self.class_applications[application].canonical_type,
            field: self.class_field_identity(field),
        }
    }
}
