//! Original declaration identities locate records while the source graph is built.

use super::*;

impl Lowerer {
    pub(crate) fn struct_id(&self, declaration: hir::SourceNominalId) -> hir::StructId {
        match self.nominal_owners[&declaration] {
            Owner::Struct(id) => id,
            _ => unreachable!("a struct application identifies a struct declaration"),
        }
    }

    pub(crate) fn enum_id(&self, declaration: hir::SourceNominalId) -> hir::EnumId {
        match self.nominal_owners[&declaration] {
            Owner::Enum(id) => id,
            _ => unreachable!("an enum application identifies an enum declaration"),
        }
    }

    pub(crate) fn source_enum_id(&self, declaration: hir::SourceNominalId) -> Option<hir::EnumId> {
        match self.nominal_owners.get(&declaration) {
            Some(Owner::Enum(id)) => Some(*id),
            Some(_) => unreachable!("an enum application identifies an enum declaration"),
            None => None,
        }
    }

    pub(crate) fn class_id(&self, declaration: hir::SourceNominalId) -> hir::ClassId {
        match self.nominal_owners[&declaration] {
            Owner::Class(id) => id,
            _ => unreachable!("a class application identifies a class declaration"),
        }
    }

    pub(crate) fn interface_id(&self, declaration: hir::SourceNominalId) -> hir::InterfaceId {
        match self.nominal_owners[&declaration] {
            Owner::Interface(id) => id,
            _ => unreachable!("an interface application identifies an interface declaration"),
        }
    }
}
