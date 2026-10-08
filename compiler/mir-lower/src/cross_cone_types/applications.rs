//! Concrete nominal applications use the same representation table as sources.

use super::*;
use hir::concrete as source;
use mir::MirTypeRepresentationV1 as Repr;

pub(super) fn append(
    local: &hir::LocalConcreteHir,
    semantics: &hir::CrossConeTypeSemanticsSectionV1,
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
    records: &mut Vec<mir::ParamFreeMirTypeExportV1>,
) -> Result<(), SourceMirTypeProductionError> {
    let module = input.module();
    for identity in module.meta.source_exact_types.iter() {
        let ExactTypeKey::NominalApplication { origin, .. } = identity.identity_record().key()
        else {
            continue;
        };
        let exact = identity.identity_record().id();
        let ty = local.exact_type_identities.type_for_identity(exact).ok_or(
            SourceMirTypeProductionError::ApplicationRepresentation(exact),
        )?;
        let fact = semantics
            .exact_facts()
            .get(exact)
            .ok_or(SourceMirTypeProductionError::MissingFacts(exact))?;
        let (representation, bases) = project(local, ty, module, identity.ty(), exact)?;
        reserve(records, 1)?;
        records.push(mir::ParamFreeMirTypeExportV1::try_new(
            mir::MirTypeBridgeAuthority {
                identities,
                foundation: input.foundation(),
            },
            exact,
            mir::MirTypeOriginV1::NominalApplication(*origin),
            facts(fact)?,
            representation,
            bases,
        )?);
    }
    Ok(())
}

fn project(
    local: &hir::LocalConcreteHir,
    source: source::TypeId,
    module: &mir::Module,
    ty: &mir::Type,
    exact: PersistentExactTypeId,
) -> Result<(Repr, mir::MirBaseAndInterfacesV1), SourceMirTypeProductionError> {
    let mismatch = || SourceMirTypeProductionError::ApplicationRepresentation(exact);
    let mut bases = mir::MirBaseAndInterfacesV1 {
        base: mir::MirBaseClassV1::None,
        interfaces: Vec::new(),
    };
    let representation = match (ty, &local.types[source].kind) {
        (mir::Type::Struct(id), source::TypeKind::Struct(source)) => {
            bases.interfaces = local.structs[*source]
                .direct_interfaces
                .iter()
                .map(|ty| local.exact_type_identities[*ty].id())
                .collect();
            let mir::StructRepresentation::Declared {
                fields,
                c_layout,
                interior_mutable,
                ..
            } = &module.structs[*id].representation
            else {
                return Err(mismatch());
            };
            let fields = fields
                .iter()
                .map(|field| {
                    Ok(mir::MirRepresentationFieldV1 {
                        field: field.identity,
                        value: representation::fields::exact(module, &field.ty)?,
                    })
                })
                .collect::<Result<Vec<_>, SourceMirTypeProductionError>>()?;
            Repr::Struct {
                fields,
                c_layout: c_layout.map_or(
                    mir::MirTypeCLayoutPolicyV1::Ordinary,
                    mir::MirTypeCLayoutPolicyV1::CLayout,
                ),
                interior_mutable: *interior_mutable,
            }
        }
        (mir::Type::Enum(id, _), source::TypeKind::Enum(source)) => {
            bases.interfaces = local.enums[*source]
                .direct_interfaces
                .iter()
                .map(|ty| local.exact_type_identities[*ty].id())
                .collect();
            Repr::Enum {
                variants: representation::fields::variants(module, &module.enums[*id].variants)?,
            }
        }
        (mir::Type::Interface(_), source::TypeKind::Interface(source)) => {
            bases.interfaces = local.interfaces[*source]
                .parents
                .iter()
                .map(|ty| local.exact_type_identities[*ty].id())
                .collect();
            Repr::Interface
        }
        (mir::Type::Class(id), source::TypeKind::Class(source)) => {
            let class = &module.classes[*id];
            let source = &local.classes[*source];
            bases.interfaces = source
                .direct_interfaces
                .iter()
                .map(|ty| local.exact_type_identities[*ty].id())
                .collect();
            match &class.representation {
                mir::ClassRepresentation::Declared { fields, base_class } => {
                    let inherited =
                        base_class.map_or(0, |base| module.classes[base].declared_fields().len());
                    let fields = fields.get(inherited..).ok_or_else(mismatch)?;
                    if fields.len() != source.declared_fields().len() {
                        return Err(mismatch());
                    }
                    let declared_fields = fields
                        .iter()
                        .zip(source.declared_fields())
                        .map(|(field, source)| {
                            Ok(mir::MirRepresentationFieldV1 {
                                field: source.identity,
                                value: representation::fields::exact(module, &field.ty)?,
                            })
                        })
                        .collect::<Result<Vec<_>, SourceMirTypeProductionError>>()?;
                    if let Some(base) = base_class {
                        bases.base = mir::MirBaseClassV1::Base(representation::fields::exact(
                            module,
                            &mir::Type::Class(*base),
                        )?);
                    }
                    bases.interfaces.sort_unstable();
                    Repr::Class {
                        release_policy: representation::release_policy(class.release_policy, exact),
                        kind: match class.modifier {
                            mir::ClassModifier::Final => mir::MirClassKindV1::Final,
                            mir::ClassModifier::Open => mir::MirClassKindV1::Open,
                            mir::ClassModifier::Abstract => mir::MirClassKindV1::Abstract,
                        },
                        declared_fields,
                    }
                }
                mir::ClassRepresentation::Intrinsic(
                    mir::IntrinsicTypeRepresentation::Array { element }
                    | mir::IntrinsicTypeRepresentation::MutableArray { element },
                ) => Repr::InlineArray {
                    element: representation::fields::exact(module, element)?,
                },
                mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Atomic(
                    hir::AtomicStorage::Reference(value),
                )) => Repr::AtomicReference {
                    value: representation::fields::exact(module, value)?,
                },
                _ => return Err(mismatch()),
            }
        }
        _ => return Err(mismatch()),
    };
    bases.interfaces.sort_unstable();
    bases.interfaces.dedup();
    Ok((representation, bases))
}
