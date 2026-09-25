use super::{LocalNominalId, owner_atom, owner_resolution, source_nominal_id, source_shape};
use crate::production::signatures::HirInterfaceSignatureProjector;
use crate::production::type_semantics::inheritance::source_errors::{invalid, resource};
use crate::*;
use scoop_identity::SourceDeclarationKey;
use scoop_wire::WirePath;

type Error = CrossConeTypeSemanticsProductionError;

mod constructors;
mod declarations;
mod dispatch_order;
pub(super) use declarations::project_required as project_required_declarations;
pub(super) use declarations::project_roots as project_root_declarations;
mod members;
mod nested;
pub(in crate::production) use nested::{NestedSourceNode, project as project_nested_sources};
mod projection;
mod roots;
pub(in crate::production) use roots::SharedSourceRoots;
mod shapes;

impl CanonicalNominalSourceContractsV1 {
    /// Projects exactly the independently required source owners from sealed HIR.
    /// Complete declaration validation must still close the required inventory,
    /// artifact identities, access, and all referenced source-support records.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        required: &CanonicalSourceNominalIdsV1,
    ) -> Result<Self, Error> {
        let export = output.module();
        let path = WirePath::root();
        let mut records = Vec::new();

        scoop_wire::allocation::try_reserve(&mut records, required.values().len(), &path)
            .map_err(resource)?;
        for local in locals(export) {
            let identity = local
                .identity(export)
                .ok_or_else(|| invalid("sealed nominal has no typed identity"))?;
            let Some(source) = identity.source() else {
                continue;
            };
            let owner = source_nominal_id(source);
            if source.declaration().origin() != export.cone
                || required.values().binary_search(&owner).is_err()
            {
                continue;
            }
            records.push(projection::project(export, local, source)?);
        }
        if records.len() != required.values().len() {
            return Err(invalid(
                "required nominal source owner is absent from this sealed HIR",
            ));
        }
        Self::try_new(records).map_err(Error::SourceInventory)
    }
}

fn locals(export: &ExportHir) -> impl Iterator<Item = LocalNominalId> + '_ {
    export
        .classes
        .iter()
        .map(|(id, _)| LocalNominalId::Class(id))
        .chain(
            export
                .interfaces
                .iter()
                .map(|(id, _)| LocalNominalId::Interface(id)),
        )
        .chain(
            export
                .structs
                .iter()
                .map(|(id, _)| LocalNominalId::Struct(id)),
        )
        .chain(export.enums.iter().map(|(id, _)| LocalNominalId::Enum(id)))
        .chain(
            export
                .objects
                .iter()
                .map(|(id, _)| LocalNominalId::Object(id)),
        )
}
fn source_owner(export: &ExportHir, owner: NominalOwner) -> Result<SourceNominalId, Error> {
    let local = match owner {
        NominalOwner::Class(id) => LocalNominalId::Class(id),
        NominalOwner::Interface(id) => LocalNominalId::Interface(id),
        NominalOwner::Struct(id) => LocalNominalId::Struct(id),
        NominalOwner::Enum(id) => LocalNominalId::Enum(id),
        NominalOwner::Object(id) => LocalNominalId::Object(id),
    };
    local
        .identity(export)
        .and_then(HirNominalIdentity::source)
        .map(source_nominal_id)
        .ok_or_else(|| invalid("nominal source has no lexical owner identity"))
}
fn validate_owner(
    export: &ExportHir,
    owner: SourceNominalId,
    key: &SourceDeclarationKey,
) -> Result<(), Error> {
    if key.origin() != export.cone || key.owners().owners().last() != Some(&owner_atom(owner)) {
        return Err(invalid(
            "nominal source member identity has a different owner",
        ));
    }
    Ok(())
}
fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), Error> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(values, 1, &path).map_err(resource)?;
    values.push(value);
    Ok(())
}

fn children(
    export: &ExportHir,
    owner: SourceNominalId,
) -> Result<CanonicalNestedNominalRefsV1, Error> {
    let mut children = Vec::new();
    for local in locals(export) {
        let identity = local
            .identity(export)
            .ok_or_else(|| invalid("nested nominal has no identity"))?;
        let Some(source) = identity.source() else {
            continue;
        };
        let key = source.declaration();
        if key.origin() == export.cone && key.owners().owners().last() == Some(&owner_atom(owner)) {
            let declared_owner = projection::header(export, local)
                .1
                .ok_or_else(|| invalid("nested source has no HIR owner"))?;
            if source_owner(export, declared_owner)? != owner {
                return Err(invalid(
                    "nested nominal source disagrees with its HIR owner",
                ));
            }
            push(&mut children, source_nominal_id(source))?;
        }
    }

    CanonicalNestedNominalRefsV1::try_new(children).map_err(invalid)
}
