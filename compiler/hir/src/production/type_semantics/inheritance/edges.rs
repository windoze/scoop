use super::*;

pub(in crate::production::type_semantics) fn project_edges(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<NominalInheritanceEdgesV1, Error> {
    project_edges_with(export, nominal, exact)
}

pub(in crate::production::type_semantics) fn project_source_edges(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<NominalInheritanceEdgesV1, Error> {
    project_edges_with(export, nominal, source_exact)
}

fn project_edges_with(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
    resolve: fn(&ExportHir, TypeId) -> Result<PersistentExactTypeId, Error>,
) -> Result<NominalInheritanceEdgesV1, Error> {
    let (modality, base, interfaces) = match nominal.local {
        NominalLocalId::Struct(id) => (
            NominalInheritanceModalityV1::Final,
            None,
            export.structs[id].interfaces.as_slice(),
        ),
        NominalLocalId::Enum(id) => (
            NominalInheritanceModalityV1::Final,
            None,
            export.enums[id].interfaces.as_slice(),
        ),
        NominalLocalId::Class(id) => {
            let class = &export.classes[id];
            let modality = match class.modifier {
                ClassModifier::Final => NominalInheritanceModalityV1::Final,
                ClassModifier::Open => NominalInheritanceModalityV1::Open,
                ClassModifier::Abstract => NominalInheritanceModalityV1::Abstract,
            };
            (modality, class.base_class, class.interfaces.as_slice())
        }
        NominalLocalId::Interface(id) => {
            let interface = &export.interfaces[id];
            let parents = interface
                .parents
                .iter()
                .map(|parent| export.interface_applications[*parent].canonical_type)
                .collect::<Vec<_>>();
            return build_edges(
                export,
                nominal.exact,
                NominalInheritanceModalityV1::Interface,
                None,
                &parents,
                resolve,
            );
        }
        NominalLocalId::Object(id) => {
            let class = &export.classes[export.objects[id].backing_class];
            (
                NominalInheritanceModalityV1::Final,
                class.base_class,
                class.interfaces.as_slice(),
            )
        }
    };
    build_edges(export, nominal.exact, modality, base, interfaces, resolve)
}

fn build_edges(
    export: &ExportHir,
    owner: PersistentExactTypeId,
    modality: NominalInheritanceModalityV1,
    base: Option<TypeId>,
    interfaces: &[TypeId],
    resolve: fn(&ExportHir, TypeId) -> Result<PersistentExactTypeId, Error>,
) -> Result<NominalInheritanceEdgesV1, Error> {
    let base = match base {
        Some(ty) => DirectClassBaseV1::ClassBase {
            exact: resolve(export, ty)?,
        },
        None => DirectClassBaseV1::NoClassBase,
    };
    let interfaces = interfaces
        .iter()
        .map(|ty| resolve(export, *ty))
        .collect::<Result<Vec<_>, _>>()?;
    NominalInheritanceEdgesV1::try_new(owner, modality, base, interfaces).map_err(|error| {
        Error::InvalidInheritance {
            exact: owner,
            reason: error.to_string(),
        }
    })
}

pub(super) fn exact(export: &ExportHir, ty: TypeId) -> Result<PersistentExactTypeId, Error> {
    let exact = source_exact(export, ty)?;
    let generic_application = match &export.types[ty] {
        Type::Struct(application) => !export.struct_applications[*application]
            .arguments
            .is_empty(),
        Type::Enum(application) => !export.enum_applications[*application].arguments.is_empty(),
        Type::Class(application) => !export.class_applications[*application].arguments.is_empty(),
        Type::Interface(application) => !export.interface_applications[*application]
            .arguments
            .is_empty(),
        _ => false,
    };
    if generic_application {
        return Err(Error::GenericOdrRequired(exact));
    }
    Ok(exact)
}

fn source_exact(export: &ExportHir, ty: TypeId) -> Result<PersistentExactTypeId, Error> {
    export
        .type_identities
        .get(ty)
        .and_then(HirTypeIdentity::exact)
        .map(|record| record.id())
        .ok_or(Error::MissingExactIdentity)
}
