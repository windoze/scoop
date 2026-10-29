//! Lower the selected static objects using their exact type metadata.
use super::*;

pub(super) fn lower_immortals(
    input: &mir::ConeMirInput,
    identity_roots: &IdentityRoots<'_>,
    selected_layout: Option<&lir::StrongProductionDependencySelectionV2<'_>>,
    arrays: &HashMap<mir::ClassId, lir::ArrayTypeId>,
) -> Result<
    (
        Arena<lir::Global>,
        HashMap<mir::ImmortalConstId, lir::GlobalId>,
    ),
    LirLoweringError,
> {
    let module = input.module();
    let mut globals = Arena::new();
    let mut immortal_global_map: HashMap<mir::ImmortalConstId, lir::GlobalId> = HashMap::new();
    for &id in input.materialization().immortals() {
        let constant = &module.immortals[id];
        let init = match constant.source {
            mir::ImmortalConstSource::CurrentCone => {
                let identity = lir::ImmortalObjectIdentity::from_key(
                    constant.identity.clone(),
                    identity_roots.for_immortal_object(&constant.identity),
                )
                .expect("validated MIR immortal identities have canonical CBOR records");
                match &constant.value {
                    mir::ImmortalConstValue::String(value) => lir::GlobalInit::StringConst {
                        identity,
                        value: value.clone(),
                    },
                    mir::ImmortalConstValue::EmbeddedFileArray { array_type, files } => {
                        lir::GlobalInit::EmbeddedFileArrayConst {
                            identity,
                            array_type: arrays[array_type],
                            files: files
                                .iter()
                                .map(|file| lir::EmbeddedFileConst {
                                    path: immortal_global_map[&file.path],
                                    bytes: immortal_global_map[&file.bytes],
                                })
                                .collect(),
                        }
                    }
                    mir::ImmortalConstValue::ByteArray { array_type, bytes } => {
                        lir::GlobalInit::ByteArrayConst {
                            identity,
                            array_type: arrays[array_type],
                            bytes: bytes.clone(),
                        }
                    }
                }
            }
            mir::ImmortalConstSource::Imported(provider) => {
                let object =
                    scoop_identity::PersistentImmortalObjectId::from_key(&constant.identity)
                        .expect("MIR immortal identities have canonical records");
                let definition = selected_layout
                    .and_then(|selected| {
                        selected
                            .physical_imports()
                            .records()
                            .iter()
                            .find(|definition| {
                                definition.provider() == provider
                                    && definition.subject()
                                        == lir::ExternalStrongShapeSubjectV1::ImmortalObject(object)
                            })
                    })
                    .ok_or(LirLoweringError::DependencyImmortalBinding { provider, object })?;
                lir::GlobalInit::ImportedImmortal {
                    definition: Box::new(definition.clone()),
                }
            }
        };
        let global = globals.alloc(lir::Global {
            address_kind: lir::PointerKind::Managed,
            scan: lir::RefScan::None,
            init,
        });
        immortal_global_map.insert(id, global);
    }

    Ok((globals, immortal_global_map))
}
