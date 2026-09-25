use std::cmp::Ordering;

use scoop_wire::{WireError, WirePath};

use super::super::{ProtectedDefaultReferenceKindV1, ProtectedDefaultReferenceV1};
use super::receiver::ProtectedDefaultReferenceReceiverV1;
use super::{ProtectedDefaultBodyClosureError, ProtectedDefaultReferenceBodySemanticAuthority};
use crate::{
    DefaultBodyReferenceOccurrenceV1, ProtectedDefaultExpressionUseV1,
    ProtectedDefaultTemplateKeyV1,
};

pub(super) struct Domain<'a, T> {
    records: &'a [ProtectedDefaultReferenceV1<T>],
    seen: Vec<Seen>,
    kind: ProtectedDefaultReferenceKindV1,
}
struct Seen {
    referenced: bool,
    uses: Vec<bool>,
}
impl<'a, T> Domain<'a, T> {
    pub fn new(
        records: &'a [ProtectedDefaultReferenceV1<T>],
        kind: ProtectedDefaultReferenceKindV1,

        path: &WirePath,
    ) -> Result<Self, WireError> {
        let mut seen = Vec::new();
        scoop_wire::allocation::try_reserve(&mut seen, records.len(), path)?;
        for record in records {
            let mut uses = Vec::new();
            scoop_wire::allocation::try_reserve(&mut uses, record.uses().values().len(), path)?;
            uses.resize(record.uses().values().len(), false);
            seen.push(Seen {
                referenced: false,
                uses,
            });
        }
        Ok(Self {
            records,
            seen,
            kind,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn observe<A: ProtectedDefaultReferenceBodySemanticAuthority<E>, E>(
        &mut self,
        key: ProtectedDefaultTemplateKeyV1,
        occurrence: DefaultBodyReferenceOccurrenceV1<'_>,
        expected_use: Option<ProtectedDefaultExpressionUseV1>,
        receiver: ProtectedDefaultReferenceReceiverV1<'_>,
        mut compare: impl FnMut(&T, &WirePath) -> Result<Ordering, WireError>,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), ProtectedDefaultBodyClosureError<E>> {
        use ProtectedDefaultBodyClosureError as Error;
        let mut low = 0;
        let mut high = self.records.len();
        while low < high {
            let index = low + (high - low) / 2;
            let record = &self.records[index];
            let mut ordering = compare(record.target(), path)?;
            if ordering == Ordering::Equal {
                ordering = occurrence.definition_origin.cmp(record.definition_origin());
            }
            match ordering {
                Ordering::Less => high = index,
                Ordering::Greater => low = index + 1,
                Ordering::Equal => {
                    if record.witness().owner() != key.owner() {
                        return Err(Error::WitnessOwner {
                            kind: self.kind,
                            index,
                        });
                    }
                    self.seen[index].referenced = true;
                    if let Some(expected) = expected_use {
                        let values = record.uses().values();

                        let found =
                            values
                                .binary_search(&expected)
                                .map_err(|_| Error::MissingUse {
                                    kind: self.kind,
                                    index,
                                    expected,
                                })?;
                        self.seen[index].uses[found] = true;
                    }
                    authority
                        .validate_default_reference_occurrence(
                            key,
                            occurrence,
                            record.witness(),
                            receiver,
                            path,
                        )
                        .map_err(Error::Source)?;
                    return Ok(());
                }
            }
        }
        Err(Error::Missing {
            kind: self.kind,
            insertion_index: low,
            site: occurrence.site,
        })
    }

    pub fn finish<E>(&self) -> Result<(), ProtectedDefaultBodyClosureError<E>> {
        use ProtectedDefaultBodyClosureError as Error;
        for (index, seen) in self.seen.iter().enumerate() {
            if !seen.referenced {
                return Err(Error::Extra {
                    kind: self.kind,
                    index,
                });
            }
            for (use_index, seen) in seen.uses.iter().enumerate() {
                if !seen {
                    return Err(Error::ExtraUse {
                        kind: self.kind,
                        index,
                        use_index,
                    });
                }
            }
        }
        Ok(())
    }
}
