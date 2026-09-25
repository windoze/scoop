//! Borrows actual nodes and joins the canonical reference set without copying trees.
use super::*;
use scoop_hir::{
    DefaultBodyReferenceTargetV1, DefaultBodyReferenceVisitorV1, DefaultExpressionV1,
    ExportDefaultCallableReferenceV1,
};

use std::cmp::Ordering;

pub(super) struct CallableOccurrence<'a> {
    pub index: usize,
    pub target: View<'a>,
    pub body: DefaultBodyReferenceOccurrenceV1<'a>,
}
type Occurrences<'a> = Vec<CallableOccurrence<'a>>;

pub(super) fn collect<'a>(
    template: &'a ExportDefaultTemplateV1,

    path: &WirePath,
) -> Result<Occurrences<'a>, Error> {
    let mut visitor = Visitor {
        records: template.references().callables(),
        occurrences: Vec::new(),
    };

    template.body().visit_direct_references(
        template.locals(),
        template.definition_origin(),
        &mut visitor,
        path,
    )?;
    Ok(visitor.occurrences)
}

struct Visitor<'a> {
    records: &'a [ExportDefaultCallableReferenceV1],
    occurrences: Occurrences<'a>,
}

impl<'a> DefaultBodyReferenceVisitorV1<'a> for Visitor<'a> {
    type Error = Error;

    fn expression(
        &mut self,
        _: u32,
        _: &'a DefaultExpressionV1,

        _: &WirePath,
    ) -> Result<(), Error> {
        // References retain their actual expression attachments in the callback below.
        Ok(())
    }

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'a>,

        path: &WirePath,
    ) -> Result<(), Error> {
        let DefaultBodyReferenceTargetV1::Callable(target) = occurrence.target else {
            return Ok(());
        };
        let mut start = 0;
        let mut end = self.records.len();
        while start < end {
            let middle = start + (end - start) / 2;
            match compare(target, occurrence, &self.records[middle], path)? {
                Ordering::Less => end = middle,
                Ordering::Greater => start = middle + 1,
                Ordering::Equal => {
                    for adjacent in [middle.checked_sub(1), middle.checked_add(1)] {
                        if let Some(record) = adjacent.and_then(|i| self.records.get(i))
                            && compare(target, occurrence, record, path)?.is_eq()
                        {
                            return Err(Error::ReferenceMatch {
                                site: occurrence.site,
                                reason: "multiple references share this actual target and origin",
                            });
                        }
                    }

                    scoop_wire::allocation::try_reserve(&mut self.occurrences, 1, path)?;
                    self.occurrences.push(CallableOccurrence {
                        index: middle,
                        target,
                        body: occurrence,
                    });
                    return Ok(());
                }
            }
        }
        Err(Error::ReferenceMatch {
            site: occurrence.site,
            reason: "actual callable target and origin have no reference record",
        })
    }
}

fn compare(
    target: View<'_>,
    occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
    record: &ExportDefaultCallableReferenceV1,

    path: &WirePath,
) -> Result<Ordering, Error> {
    let ordering = target.compare_to(record.target(), path)?;
    if !ordering.is_eq() {
        return Ok(ordering);
    }

    Ok(occurrence.definition_origin.cmp(record.definition_origin()))
}
