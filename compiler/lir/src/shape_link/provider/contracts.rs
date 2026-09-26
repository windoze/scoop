use super::*;
use scoop_identity::ScanRole;

impl<'a> ShapeLinkProviderV1<'a> {
    pub(super) fn contract(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        physical: StrongShapeDefinitionRefV1,
        support: &dyn ShapeLinkSupportLookupV1<'a>,
    ) -> Result<ShapeLinkContractV1, ShapeLinkError> {
        use ExternalStrongShapeSubjectV1 as Subject;

        Ok(match subject {
            Subject::Callable(target) => {
                let record = self
                    .parts
                    .callables
                    .get(target)
                    .ok_or(ShapeLinkError::MissingSubject(subject))?;

                let registration = self
                    .parts
                    .production
                    .callable_registrations()
                    .registrations()
                    .iter()
                    .find(|registration| registration.body() == record.definition().semantic_id())
                    .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
                if record.physical_definition() != physical
                    || registration.body_definition_plan() != physical.definition()
                    || registration.entry_symbol() != physical.symbol()
                    || registration.body_primary_atom() != physical.primary()
                {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
                ShapeLinkContractV1::CallableAbi {
                    canonical_signature: record.canonical_signature().clone(),
                    calling_convention: record.calling_convention(),
                    protocol: record.call_protocol(),
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
                    (ExactLayoutBodyKindV1::Instance(instance), ScanRole::ArrayElement)
                        if matches!(
                            instance.representation().kind(),
                            InstanceRepresentationKindV1::InlineArray { .. }
                        ) =>
                    {
                        instance.shape().inline_scan()
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
            | Subject::InitializationDescriptor(_) => {
                self.support_contract(subject, physical, support)?
            }
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
