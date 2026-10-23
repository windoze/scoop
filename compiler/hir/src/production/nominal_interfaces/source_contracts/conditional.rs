use super::*;
use scoop_identity::{NonEmptyVec, SignatureTypeKey};

pub(super) fn conformances(
    export: &ExportHir,
    local: LocalNominalId,
    signatures: &HirInterfaceSignatureProjector<'_>,
    binders: &[HirSignatureBinder],
) -> Result<Vec<ConditionalInterfaceConformanceV1>, Error> {
    let implementations = match local {
        LocalNominalId::Class(id) => &export.classes[id].interface_implementations,
        LocalNominalId::Object(id) => {
            &export.classes[export.objects[id].backing_class].interface_implementations
        }
        LocalNominalId::Struct(id) => &export.structs[id].interface_implementations,
        LocalNominalId::Enum(id) => &export.enums[id].interface_implementations,
        LocalNominalId::Interface(_) => return Ok(Vec::new()),
    };
    implementations
        .iter()
        .filter_map(|implementation| {
            let condition = implementation.condition.as_ref()?;
            Some((|| {
                Ok(ConditionalInterfaceConformanceV1 {
                    interface: signatures
                        .map_type(implementation.interface, binders)
                        .map_err(invalid)?,
                    condition: ConditionalInterfaceApplicationV1 {
                        declaration: condition.declaration,
                        owner: match condition.owner {
                            SourceNominalId::Concrete(owner) => SignatureTypeKey::Nominal(owner),
                            SourceNominalId::GenericTemplate(origin) => {
                                SignatureTypeKey::NominalApplication {
                                    origin,
                                    arguments: NonEmptyVec::new(
                                        condition
                                            .arguments
                                            .iter()
                                            .map(|&argument| {
                                                signatures
                                                    .map_type(argument, binders)
                                                    .map_err(invalid)
                                            })
                                            .collect::<Result<_, _>>()?,
                                    )
                                    .map_err(invalid)?,
                                }
                            }
                        },
                    },
                })
            })())
        })
        .collect()
}
