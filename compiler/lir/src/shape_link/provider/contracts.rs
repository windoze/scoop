use super::*;
use scoop_identity::ScanRole;

impl<'a> ShapeLinkProviderV1<'a> {
    pub(in crate::shape_link) fn contract(
        &self,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<ShapeLinkContractV1, ShapeLinkError> {
        use ExternalStrongShapeSubjectV1 as Subject;

        Ok(match subject {
            Subject::ImmortalObject(_) => ShapeLinkContractV1::ImmortalObject,
            Subject::Callable(target) => {
                let (signature, calling_convention, protocol) = match (
                    self.parts.callables.get(target),
                    target
                        .strong_owner()
                        .and_then(|target| self.parts.ordinary.export_for_target(target)),
                ) {
                    (Some(record), None) => (
                        record.canonical_signature(),
                        record.calling_convention(),
                        record.call_protocol(),
                    ),
                    (None, Some(record)) => {
                        let protocol = match record.root_plan() {
                            crate::ExternalCallableRootPlan::ManagedStatepoint => {
                                crate::ExactCallableProtocolV1::OrdinaryManaged
                            }
                            crate::ExternalCallableRootPlan::NoGc => {
                                crate::ExactCallableProtocolV1::OrdinaryNoGc
                            }
                        };
                        (
                            record.abi_signature(),
                            record.calling_convention(),
                            protocol,
                        )
                    }
                    (Some(_), Some(_)) => return Err(ShapeLinkError::DefinitionRelation(subject)),
                    (None, None) => return Err(ShapeLinkError::MissingSubject(subject)),
                };
                ShapeLinkContractV1::CallableAbi {
                    canonical_signature: signature.clone(),
                    calling_convention,
                    protocol,
                }
            }
            Subject::Layout(layout) => {
                let record = self
                    .parts
                    .layouts
                    .get(layout)
                    .ok_or(ShapeLinkError::MissingSubject(subject))?;
                ShapeLinkContractV1::Layout {
                    record: record.clone(),
                }
            }
            Subject::Scan(scan) => {
                let binding = self
                    .parts
                    .foundation
                    .scans()
                    .iter()
                    .find(|record| record.id() == scan)
                    .ok_or(ShapeLinkError::MissingSubject(subject))?;

                let record = self
                    .parts
                    .layouts
                    .get(binding.key().layout())
                    .ok_or(ShapeLinkError::MissingSubject(subject))?;
                let canonical_scan = match (record.kind(), binding.key().role()) {
                    (ExactLayoutBodyKindV1::Value(value), ScanRole::InlineValue) => {
                        storage_scan(value.value().storage())
                    }
                    (ExactLayoutBodyKindV1::Instance(instance), ScanRole::ManagedObject) => {
                        instance.shape().object_scan()
                    }
                    _ => return Err(ShapeLinkError::DefinitionRelation(subject)),
                };
                ShapeLinkContractV1::Scan {
                    layout: record.identity().layout(),
                    role: binding.key().role(),
                    canonical_scan: canonical_scan.clone(),
                }
            }
            Subject::TypeDescriptor(exact) | Subject::TypeRegistration(exact) => {
                ShapeLinkContractV1::Type {
                    descriptor_projection: self
                        .parts
                        .descriptors
                        .get(exact)
                        .ok_or(ShapeLinkError::MissingSubject(subject))?
                        .clone(),
                }
            }
            Subject::DispatchTable(table) => ShapeLinkContractV1::Dispatch {
                table_projection: self
                    .parts
                    .dispatch
                    .get(table)
                    .ok_or(ShapeLinkError::MissingSubject(subject))?
                    .clone(),
            },
            Subject::StaticStorage(_)
            | Subject::StaticStorageRegistration(_)
            | Subject::InitializationCell(_)
            | Subject::InitializationRegistration(_) => self.support_contract(subject)?,
        })
    }
}

pub(in crate::shape_link) fn storage_scan(storage: &ValueStorageLayoutV1) -> &RefScan {
    static NONE: RefScan = RefScan::None;
    match storage.kind() {
        ValueStorageKindV1::ZeroSized { .. } => &NONE,
        ValueStorageKindV1::Inline { scan, .. } => scan.as_ref_scan(),
    }
}
