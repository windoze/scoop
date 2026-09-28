use super::*;

pub(in crate::production::type_semantics) fn project_edges(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
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
            return build_edges(
                export,
                nominal.exact,
                NominalInheritanceModalityV1::Interface,
                None,
                &interface.parents,
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
    build_edges(export, nominal.exact, modality, base, interfaces)
}

fn build_edges(
    export: &ExportHir,
    owner: PersistentExactTypeId,
    modality: NominalInheritanceModalityV1,
    base: Option<TypeId>,
    interfaces: &[TypeId],
) -> Result<NominalInheritanceEdgesV1, Error> {
    let base = match base {
        Some(ty) => DirectClassBaseV1::ClassBase {
            exact: exact(export, ty)?,
        },
        None => DirectClassBaseV1::NoClassBase,
    };
    let interfaces = interfaces
        .iter()
        .map(|ty| exact(export, *ty))
        .collect::<Result<Vec<_>, _>>()?;
    NominalInheritanceEdgesV1::try_new(owner, modality, base, interfaces).map_err(|error| {
        Error::InvalidInheritance {
            exact: owner,
            reason: error.to_string(),
        }
    })
}

pub(super) fn exact(export: &ExportHir, ty: TypeId) -> Result<PersistentExactTypeId, Error> {
    export
        .type_identities
        .get(ty)
        .and_then(HirTypeIdentity::exact)
        .map(|record| record.id())
        .ok_or_else(|| {
            Error::InvalidSourceDeclaration(format!(
                "inheritance signature has no exact type identity: {:?}",
                export.types[ty]
            ))
        })
}
