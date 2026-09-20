//! Projection of the complete public HIR nominal surface.

use std::collections::HashSet;

use scoop_identity::{DefinitionOwnerAtom, NominalDeclarationOwner};

use super::signatures::HirInterfaceSignatureProjector;
use crate::{
    CanonicalNominalInterfacesV1, ExportHir, HirSignatureBinder, NominalInterfaceRecordV1,
};

mod constructors;
mod errors;
mod identity;
mod members;
mod nested_bindings;
pub(in crate::production) mod owner_resolution;
mod source_contracts;
mod source_shape;

pub use errors::{
    NominalArenaKind, NominalConstructorProjectionError, NominalInterfaceBuildError,
    NominalMemberProjectionError, NominalNestedBindingProjectionError,
    NominalSourceProjectionError,
};

struct NominalProjection<'a> {
    export: &'a ExportHir,
    signatures: HirInterfaceSignatureProjector<'a>,
    public_functions: HashSet<crate::FunctionId>,
    public_properties: HashSet<crate::PropertyId>,
    public_struct_constructors: HashSet<crate::StructConstructorId>,
    public_class_constructors: HashSet<crate::ClassConstructorId>,
}

struct ProjectedNominalHeader {
    declaration: NominalDeclarationOwner,
    type_parameters: crate::CanonicalBinderListV1,
    binders: Vec<HirSignatureBinder>,
}

use identity::{LocalNominalId, source_nominal_id};

impl CanonicalNominalInterfacesV1 {
    /// Projects exactly the current Cone's foreign-public nominal lookup
    /// surface. Member and nested relations remain attached to their typed
    /// source owner instead of being recovered from names.
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, NominalInterfaceBuildError> {
        let projection = NominalProjection::new(export);
        let mut records = Vec::with_capacity(
            export.public_surface.classes.len()
                + export.public_surface.interfaces.len()
                + export.public_surface.structs.len()
                + export.public_surface.enums.len()
                + export.public_surface.objects.len(),
        );
        for &id in &export.public_surface.classes {
            records.push(projection.project_class(id)?);
        }
        for &id in &export.public_surface.interfaces {
            records.push(projection.project_interface(id)?);
        }
        for &id in &export.public_surface.structs {
            records.push(projection.project_struct(id)?);
        }
        for &id in &export.public_surface.enums {
            records.push(projection.project_enum(id)?);
        }
        for &id in &export.public_surface.objects {
            records.push(projection.project_object(id)?);
        }
        Self::try_new(records).map_err(NominalInterfaceBuildError::Table)
    }
}

impl<'a> NominalProjection<'a> {
    fn new(export: &'a ExportHir) -> Self {
        Self {
            export,
            signatures: HirInterfaceSignatureProjector::new(export),
            public_functions: export.public_surface.functions.iter().copied().collect(),
            public_properties: export.public_surface.properties.iter().copied().collect(),
            public_struct_constructors: export
                .public_surface
                .struct_constructors
                .iter()
                .copied()
                .collect(),
            public_class_constructors: export
                .public_surface
                .class_constructors
                .iter()
                .copied()
                .collect(),
        }
    }

    fn project_class(
        &self,
        id: crate::ClassId,
    ) -> Result<NominalInterfaceRecordV1, NominalInterfaceBuildError> {
        let declaration = arena_get(&self.export.classes, id)
            .ok_or_else(|| self.unknown_public(LocalNominalId::Class(id)))?;
        let header = self.project_header(
            LocalNominalId::Class(id),
            &declaration.name,
            declaration.owner,
            &declaration.access,
            &declaration.type_params,
        )?;
        let exact_supertypes = source_shape::class_supertypes(
            &source_shape::SourceShapeProjection::new(self.export),
            header.declaration,
            declaration,
            &header.binders,
        )?;
        let owner = header.declaration;
        self.finish_record(
            LocalNominalId::Class(id),
            header,
            exact_supertypes,
            constructors::from_class(self, id, declaration, owner)?,
            members::ordinary(self, owner, &declaration.methods, &declaration.properties)?,
            crate::NominalSourceShapeV1::Class,
        )
    }

    fn project_interface(
        &self,
        id: crate::InterfaceId,
    ) -> Result<NominalInterfaceRecordV1, NominalInterfaceBuildError> {
        let declaration = arena_get(&self.export.interfaces, id)
            .ok_or_else(|| self.unknown_public(LocalNominalId::Interface(id)))?;
        let header = self.project_header(
            LocalNominalId::Interface(id),
            &declaration.name,
            declaration.owner,
            &declaration.access,
            &declaration.type_params,
        )?;
        let exact_supertypes = source_shape::interface_supertypes(
            &source_shape::SourceShapeProjection::new(self.export),
            header.declaration,
            declaration,
            &header.binders,
        )?;
        let owner = header.declaration;
        self.finish_record(
            LocalNominalId::Interface(id),
            header,
            exact_supertypes,
            constructors::none(),
            members::interface(self, id, declaration, owner)?,
            crate::NominalSourceShapeV1::Interface,
        )
    }

    fn project_struct(
        &self,
        id: crate::StructId,
    ) -> Result<NominalInterfaceRecordV1, NominalInterfaceBuildError> {
        let declaration = arena_get(&self.export.structs, id)
            .ok_or_else(|| self.unknown_public(LocalNominalId::Struct(id)))?;
        let header = self.project_header(
            LocalNominalId::Struct(id),
            &declaration.name,
            declaration.owner,
            &declaration.access,
            &declaration.type_params,
        )?;
        let exact_supertypes = source_shape::direct_supertypes(
            &source_shape::SourceShapeProjection::new(self.export),
            header.declaration,
            &declaration.interfaces,
            &header.binders,
        )?;
        let shape = source_shape::struct_shape(
            &source_shape::SourceShapeProjection::new(self.export),
            id,
            declaration,
            &header.binders,
            header.declaration,
        )?;
        let owner = header.declaration;
        self.finish_record(
            LocalNominalId::Struct(id),
            header,
            exact_supertypes,
            constructors::from_struct(self, id, declaration, owner)?,
            members::ordinary(self, owner, &declaration.methods, &declaration.properties)?,
            shape,
        )
    }

    fn project_enum(
        &self,
        id: crate::EnumId,
    ) -> Result<NominalInterfaceRecordV1, NominalInterfaceBuildError> {
        let declaration = arena_get(&self.export.enums, id)
            .ok_or_else(|| self.unknown_public(LocalNominalId::Enum(id)))?;
        let header = self.project_header(
            LocalNominalId::Enum(id),
            &declaration.name,
            declaration.owner,
            &declaration.access,
            &declaration.type_params,
        )?;
        let exact_supertypes = source_shape::direct_supertypes(
            &source_shape::SourceShapeProjection::new(self.export),
            header.declaration,
            &declaration.interfaces,
            &header.binders,
        )?;
        let shape = source_shape::enum_shape(
            &source_shape::SourceShapeProjection::new(self.export),
            id,
            declaration,
            &header.binders,
            header.declaration,
        )?;
        let owner = header.declaration;
        self.finish_record(
            LocalNominalId::Enum(id),
            header,
            exact_supertypes,
            constructors::none(),
            members::ordinary(self, owner, &declaration.methods, &declaration.properties)?,
            shape,
        )
    }

    fn project_object(
        &self,
        id: crate::ObjectId,
    ) -> Result<NominalInterfaceRecordV1, NominalInterfaceBuildError> {
        let declaration = arena_get(&self.export.objects, id)
            .ok_or_else(|| self.unknown_public(LocalNominalId::Object(id)))?;
        let header = self.project_header(
            LocalNominalId::Object(id),
            &declaration.name,
            declaration.owner,
            &declaration.access,
            &[],
        )?;
        let backing = arena_get(&self.export.classes, declaration.backing_class).ok_or(
            NominalInterfaceBuildError::UnknownLexicalOwner {
                declaration: header.declaration,
                owner: NominalArenaKind::Class,
                index: raw_index(declaration.backing_class),
            },
        )?;
        let exact_supertypes = source_shape::object_supertypes(
            &source_shape::SourceShapeProjection::new(self.export),
            header.declaration,
            backing,
            &header.binders,
        )?;
        let members = members::ordinary(
            self,
            header.declaration,
            &backing.methods,
            &backing.properties,
        )?;
        let shape = source_shape::object_shape(
            &source_shape::SourceShapeProjection::new(self.export),
            id,
            declaration,
            header.declaration,
        )?;
        self.finish_record(
            LocalNominalId::Object(id),
            header,
            exact_supertypes,
            constructors::none(),
            members,
            shape,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_record(
        &self,
        local: LocalNominalId,
        header: ProjectedNominalHeader,
        exact_supertypes: crate::CanonicalSignatureTypesV1,
        constructors: crate::CanonicalPersistentIdsV1<scoop_identity::PersistentConstructorId>,
        members: crate::CanonicalPublicMemberRefsV1,
        source_shape: crate::NominalSourceShapeV1,
    ) -> Result<NominalInterfaceRecordV1, NominalInterfaceBuildError> {
        let nested_bindings = nested_bindings::project(self, local, header.declaration)?;
        NominalInterfaceRecordV1::try_new(
            header.declaration,
            local.kind(),
            header.type_parameters,
            exact_supertypes,
            constructors,
            members,
            nested_bindings,
            source_shape,
        )
        .map_err(|source| NominalInterfaceBuildError::Record {
            declaration: header.declaration,
            source,
        })
    }

    fn unknown_public(&self, local: LocalNominalId) -> NominalInterfaceBuildError {
        let (kind, index) = local.location();
        NominalInterfaceBuildError::UnknownPublicNominal { kind, index }
    }
}

fn owner_atom(owner: NominalDeclarationOwner) -> DefinitionOwnerAtom {
    match owner {
        NominalDeclarationOwner::Concrete(id) => DefinitionOwnerAtom::Type(id),
        NominalDeclarationOwner::GenericTemplate(id) => DefinitionOwnerAtom::GenericType(id),
    }
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((raw_index(id) as usize) < arena.len()).then(|| &arena[id])
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
