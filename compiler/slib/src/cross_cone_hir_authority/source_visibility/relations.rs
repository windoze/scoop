use super::*;
use std::collections::BTreeSet;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(in crate::cross_cone_hir_authority) fn source_domain_is_subset(
        &mut self,
        narrow: &scoop_hir::SourceAccessDomainV1,
        wide: &scoop_hir::SourceAccessDomainV1,
        path: &WirePath,
    ) -> Result<bool, Error> {
        self.meter.charge_work(1, path).map_err(Error::Resource)?;
        if narrow.is_empty() || wide.is_universal() {
            return Ok(true);
        }
        if wide.is_empty() {
            return Ok(false);
        }
        for required in wide.constraints() {
            let mut covered = false;
            for provided in narrow.constraints() {
                let cost = [&required, &provided]
                    .into_iter()
                    .map(|constraint| match constraint {
                        Constraint::File(source) => {
                            source.logical_path().as_str().len() as u64 + 65
                        }
                        _ => 65,
                    })
                    .sum();
                self.meter
                    .charge_work(cost, path)
                    .map_err(Error::Resource)?;
                if self.visibility_implies(provided, required)? {
                    covered = true;
                    break;
                }
            }
            if !covered {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn visibility_implies(
        &mut self,
        narrow: &Constraint,
        wide: &Constraint,
    ) -> Result<bool, Error> {
        if narrow == wide {
            return Ok(true);
        }
        match (narrow, wide) {
            (Constraint::File(file), Constraint::Cone(cone)) => Ok(file.cone() == *cone),
            (Constraint::LexicalOwner(owner), Constraint::Cone(cone)) => {
                Ok(self.visibility_nominal(*owner)?.1.origin() == *cone)
            }
            (Constraint::LexicalOwner(owner), Constraint::File(file)) => {
                let (_, key) = self.visibility_nominal(*owner)?;
                Ok(self.visibility_source(key.origin(), nominal_subject(*owner))? == file)
            }
            (Constraint::LexicalOwner(inner), Constraint::LexicalOwner(outer)) => {
                let (_, key) = self.visibility_nominal(*inner)?;
                for atom in key.owners().owners() {
                    self.visibility_work(1)?;
                    if nominal_owner(atom)? == *outer {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            (Constraint::SubclassesOf(derived), Constraint::SubclassesOf(base)) => {
                self.visibility_subclass(*derived, *base)
            }
            (Constraint::LexicalOwner(owner), Constraint::SubclassesOf(base)) => {
                let (_, key) = self.visibility_nominal(*owner)?;
                if self.visibility_scope_class(*owner, *base)? {
                    return Ok(true);
                }
                for atom in key.owners().owners() {
                    self.visibility_work(1)?;
                    if self.visibility_scope_class(nominal_owner(atom)?, *base)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn visibility_scope_class(
        &mut self,
        owner: SourceNominalId,
        base: SourceNominalId,
    ) -> Result<bool, Error> {
        let (record, _) = self.visibility_nominal(owner)?;
        if matches!(
            record.kind(),
            PublicNominalKindV1::Class | PublicNominalKindV1::Object
        ) {
            self.visibility_subclass(owner, base)
        } else {
            Ok(false)
        }
    }

    fn visibility_subclass(
        &mut self,
        mut derived: SourceNominalId,
        base: SourceNominalId,
    ) -> Result<bool, Error> {
        let mut visited = BTreeSet::new();
        let mut contains = false;
        loop {
            let path = WirePath::root();
            self.meter
                .check_semantic_depth(visited.len() as u64 + 1, &path)
                .map_err(Error::Resource)?;
            self.meter
                .charge_collection_slots(1, &path)
                .map_err(Error::Resource)?;
            self.meter.charge_nodes(1, &path).map_err(Error::Resource)?;
            self.visibility_work((u64::from(visited.len().max(1).ilog2()) + 1) * 65)?;
            if !visited.insert(derived) {
                return Err(Error::NominalDeclaration {
                    declaration: derived,
                    reason: "cyclic source class inheritance in visibility query",
                });
            }
            contains |= derived == base;
            let (record, _) = self.visibility_nominal(derived)?;
            let mut parent = None;
            for signature in record.exact_supertypes().values() {
                self.meter.charge_edges(1, &path).map_err(Error::Resource)?;
                let owner = match signature {
                    SignatureTypeKey::Nominal(id) => SourceNominalId::Concrete(*id),
                    SignatureTypeKey::NominalApplication { origin, .. } => {
                        SourceNominalId::GenericTemplate(*origin)
                    }
                    _ => {
                        return Err(Error::NominalDeclaration {
                            declaration: derived,
                            reason: "non-nominal source superclass",
                        });
                    }
                };
                let (record, _) = self.visibility_nominal(owner)?;
                if record.kind() == PublicNominalKindV1::Class && parent.replace(owner).is_some() {
                    return Err(Error::NominalDeclaration {
                        declaration: derived,
                        reason: "multiple source superclasses",
                    });
                }
            }
            match parent {
                Some(next) => derived = next,
                None => return Ok(contains),
            }
        }
    }
}
