use super::*;
use std::cell::RefCell;

pub(super) fn validate(
    bound: &BoundDefaultSourceAccessDeclarationsV1<'_, '_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    for (index, record) in bound.source.records().iter().enumerate() {
        let subject = record.subject();
        let path = WirePath::root().index(index as u64);
        let key = bound.source_key(subject, meter)?;
        let access = record.declaration_access();
        let owners = access.lexical_owners().len() as u64;
        meter.check_semantic_depth(owners + 1, &path)?;
        meter.check_table_entries(owners, &path)?;
        meter.charge_work((owners + 1).saturating_pow(2).saturating_mul(65), &path)?;
        charge_access(access, meter, &path)?;
        let mut replay = Replay {
            bound,
            subject,
            meter: RefCell::new(&mut *meter),
            path: &path,
        };
        access
            .validate_for_declaration(key, &mut replay)
            .map_err(|e| Error::access(subject, e))?;
        let nominal = match subject {
            Subject::Type(id) => Some(SourceNominalId::Concrete(id)),
            Subject::GenericType(id) => Some(SourceNominalId::GenericTemplate(id)),
            _ => None,
        };
        if let Some(owner) = nominal {
            let meter = replay.meter.get_mut();
            let sources = &bound.foundation.source().entries().sources;
            query(sources.records().len(), meter, &path)?;
            if let Some(existing) = sources.get(owner) {
                charge_access(existing.access(), meter, &path)?;
                if existing.access() != access {
                    return Err(Error::NominalOverlap(owner));
                }
            }
        }
    }
    Ok(())
}
fn charge_access(
    access: &DeclarationAccessSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let bytes = scoop_wire::encoded_length(access).map_err(|e| Error::Encoding(e.to_string()))?;
    meter.check_semantic_leaf(
        access
            .definition_origin()
            .origin()
            .source()
            .logical_path()
            .as_str()
            .len() as u64,
        path,
    )?;
    meter.charge_work(bytes, path)?;
    Ok(())
}
struct Replay<'b, 's, 'a, 'f, 'm> {
    bound: &'b BoundDefaultSourceAccessDeclarationsV1<'s, 'a, 'f>,
    subject: Subject,
    meter: RefCell<&'m mut BudgetMeter>,
    path: &'b WirePath,
}
impl ExportDefinitionSourceSemanticAuthority<Error> for Replay<'_, '_, '_, '_, '_> {
    fn current_cone(&self) -> ConeIdentity {
        self.bound.provider()
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), Error> {
        let foundation = self.bound.foundation;
        let meter = self.meter.get_mut();
        foundation.validate_origin(source, meter, self.path)?;
        query(
            foundation
                .foundation
                .as_canonical()
                .counts()
                .definition_origins,
            meter,
            self.path,
        )?;
        let expected = foundation
            .foundation
            .definition_origin(self.subject)
            .ok_or(Error::Origin(self.subject))?;
        meter.charge_work(
            expected.origin().source().logical_path().as_str().len() as u64 + 65,
            self.path,
        )?;
        if expected.origin() != source.origin() {
            return Err(Error::Origin(self.subject));
        }
        Ok(())
    }
}
impl DeclarationAccessSourceSemanticAuthority<Error> for Replay<'_, '_, '_, '_, '_> {
    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, Error> {
        let mut meter = self.meter.borrow_mut();
        let key = self.bound.source_key(nominal_subject(owner), &mut meter)?;
        NominalRepresentationSupportV1::charge_source_key_resources(key, &mut meter, self.path)?;
        let bytes = scoop_wire::encoded_length(key).map_err(|e| Error::Encoding(e.to_string()))?;
        // DeclarationAccessSourceV1 re-derives each nominal owner identity.
        meter.charge_sha256(bytes, self.path)?;
        Ok(key)
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, Error> {
        let mut meter = self.meter.borrow_mut();
        let record = self.bound.declaration(nominal_subject(owner), &mut meter)?;
        let origin = record.declaration_access().definition_origin();
        meter.charge_work(
            origin.origin().source().logical_path().as_str().len() as u64 + 65,
            self.path,
        )?;
        Ok(origin)
    }
}
