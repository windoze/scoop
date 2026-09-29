use super::*;
use scoop_identity::ScanRole;

impl<'a> ShapeLinkProviderV1<'a> {
    pub(super) fn contract(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        physical: StrongShapeDefinitionRefV1,
    ) -> Result<ShapeLinkContractV1, ShapeLinkError> {
        use ExternalStrongShapeSubjectV1 as Subject;

        Ok(match subject {
            Subject::Callable(target) => {
                let (signature, calling_convention, protocol, body) = match (
                    self.parts.callables.get(target),
                    target
                        .strong_owner()
                        .and_then(|target| self.parts.ordinary.export_for_target(target)),
                ) {
                    (Some(record), None) => {
                        if record.physical_definition() != physical {
                            return Err(ShapeLinkError::DefinitionRelation(subject));
                        }
                        (
                            record.canonical_signature(),
                            record.calling_convention(),
                            record.call_protocol(),
                            record.definition().semantic_id(),
                        )
                    }
                    (None, Some(record)) => {
                        let scoop_identity::PersistentSymbolKey::CallableBody(body) =
                            record.expected_symbol().key()
                        else {
                            return Err(ShapeLinkError::DefinitionRelation(subject));
                        };
                        if record.required_definition() != physical.definition()
                            || record.expected_symbol() != physical.symbol()
                        {
                            return Err(ShapeLinkError::DefinitionRelation(subject));
                        }
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
                            body,
                        )
                    }
                    (Some(_), Some(_)) => return Err(ShapeLinkError::DefinitionRelation(subject)),
                    (None, None) => return Err(ShapeLinkError::MissingSubject(subject)),
                };
                let registration = self
                    .parts
                    .production
                    .callable_registrations()
                    .registrations()
                    .iter()
                    .find(|registration| registration.body() == body)
                    .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
                if registration.body_definition_plan() != physical.definition()
                    || registration.entry_symbol() != physical.symbol()
                    || registration.body_primary_atom() != physical.primary()
                {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
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
                if record.identity().physical_definition() != physical {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
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
                if record.scan() != scan || record.scan_definition() != physical {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
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
                    descriptor_projection: self.descriptor(exact, subject, physical)?.clone(),
                }
            }
            Subject::DispatchTable(table) => ShapeLinkContractV1::Dispatch {
                table_projection: self.dispatch(table, physical)?.clone(),
            },
            Subject::StaticStorage(_)
            | Subject::StaticStorageRegistration(_)
            | Subject::InitializationCell(_)
            | Subject::InitializationDescriptor(_) => self.support_contract(subject, physical)?,
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
