//! Declaration locations are recorded when source declarations are allocated.

use super::*;
use crate::Owner;

impl Lowerer {
    pub(crate) fn record_function_file(&mut self, function: hir::FunctionId, file: usize) {
        self.function_files.insert(function, file);
        if let Some(source) = self.generated_source_file {
            self.generation.function_sources.insert(function, source);
        }
    }

    pub(crate) fn record_nominal_file(&mut self, owner: Owner, file: usize) {
        match owner {
            Owner::Class(id) => {
                self.class_files.insert(id, file);
            }
            Owner::Struct(id) => {
                self.struct_files.insert(id, file);
            }
            Owner::Enum(id) => {
                self.enum_files.insert(id, file);
            }
            Owner::Interface(id) => {
                self.interface_files.insert(id, file);
            }
            Owner::Object(id) => {
                self.object_files.insert(id, file);
            }
        }
        if let Some(source) = self.generated_source_file {
            self.generation.nominal_sources.insert(owner, source);
        }
    }

    pub(crate) fn function_definition_file(&self, function: hir::FunctionId) -> Option<usize> {
        self.generation
            .function_sources
            .get(&function)
            .or_else(|| self.function_files.get(&function))
            .copied()
    }

    pub(crate) fn nominal_definition_file(&self, owner: Owner) -> Option<usize> {
        self.generation
            .nominal_sources
            .get(&owner)
            .or_else(|| match owner {
                Owner::Class(id) => self.class_files.get(&id),
                Owner::Struct(id) => self.struct_files.get(&id),
                Owner::Enum(id) => self.enum_files.get(&id),
                Owner::Interface(id) => self.interface_files.get(&id),
                Owner::Object(id) => self.object_files.get(&id),
            })
            .copied()
    }

    pub(crate) fn property_definition_file(&self, property: hir::PropertyId) -> Option<usize> {
        let owner = match self.properties[property].owner {
            hir::PropertyOwner::Class(id) => Owner::Class(id),
            hir::PropertyOwner::Struct(id) => Owner::Struct(id),
            hir::PropertyOwner::Enum(id) => Owner::Enum(id),
            hir::PropertyOwner::Interface(id) => Owner::Interface(id),
            hir::PropertyOwner::Object(id) => Owner::Object(id),
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => {
                return self.property_source_file(property);
            }
        };
        self.nominal_definition_file(owner)
    }
}
