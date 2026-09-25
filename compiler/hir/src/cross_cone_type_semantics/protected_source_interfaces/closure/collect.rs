use super::{source::Source, *};
use crate::{
    CanonicalNominalInheritanceInterfacesV1, CanonicalProtectedDeclarationInterfacesV1,
    NestedSourceSupportV1, ProtectedDeclarationInterfaceV1, ProtectedNestedSourceInterfaceV1,
};

pub(super) fn sources<'a, E>(
    protected: &'a CanonicalProtectedDeclarationInterfacesV1,
    inheritance: &'a CanonicalNominalInheritanceInterfacesV1,
) -> Result<Vec<Source<'a>>, ProtectedSourceClosureError<E>> {
    use ProtectedSourceClosureError as Error;
    let path = WirePath::root();
    let mut records = Vec::new();
    let mut pending: Vec<&ProtectedNestedSourceInterfaceV1> = Vec::new();
    reserve(&mut records, protected.records().len())?;
    scoop_wire::allocation::try_reserve(&mut pending, protected.records().len(), &path)
        .map_err(Error::Resource)?;
    for record in protected.records() {
        match record {
            ProtectedDeclarationInterfaceV1::Callable(record) => {
                add(&mut records, Source::ProtectedCallable(record))
            }
            ProtectedDeclarationInterfaceV1::Constructor(record) => {
                add(&mut records, Source::ProtectedConstructor(record))
            }
            ProtectedDeclarationInterfaceV1::Property(_) => {}
            ProtectedDeclarationInterfaceV1::NestedNominal(record) => {
                pending.push(record.payload().source_interface())
            }
        }
    }
    for owner in inheritance.records() {
        reserve(&mut records, owner.constructors().records().len())?;
        for constructor in owner.constructors().records() {
            records.push(Source::SupportConstructor(constructor.source()));
        }
    }
    while let Some(source) = pending.pop() {
        let support = source.source_support().records();
        reserve(&mut records, support.len())?;
        scoop_wire::allocation::try_reserve(&mut pending, support.len(), &path)
            .map_err(Error::Resource)?;
        for entry in support {
            match entry {
                NestedSourceSupportV1::Callable(record) => {
                    add(&mut records, Source::SupportCallable(record))
                }
                NestedSourceSupportV1::Constructor(record) => {
                    add(&mut records, Source::SupportConstructor(record))
                }
                NestedSourceSupportV1::Property(_) => {}
                NestedSourceSupportV1::NestedNominal(record) => {
                    pending.push(record.payload().source_interface());
                }
            }
        }
    }

    records.sort_unstable_by_key(Source::owner);
    for pair in records.windows(2) {
        if pair[0].owner() != pair[1].owner() {
            continue;
        }

        if pair[0].payload() != pair[1].payload() || pair[0].access() != pair[1].access() {
            return Err(Error::ConflictingSource(pair[0].owner()));
        }
    }
    records.dedup_by_key(|record| record.owner());
    Ok(records)
}
fn add<'a>(records: &mut Vec<Source<'a>>, record: Source<'a>) {
    if !matches!(record.owner(), CallableTemplateOrigin::Accessor(_)) {
        records.push(record);
    }
}
fn reserve<E>(
    records: &mut Vec<Source<'_>>,
    count: usize,
) -> Result<(), ProtectedSourceClosureError<E>> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(records, count, &path)
        .map_err(ProtectedSourceClosureError::Resource)
}
