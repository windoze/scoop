use super::records::Domain;
use super::*;
use DefaultSourceReferenceClosureError as Error;
use DefaultSourceReferenceRecordV1 as Record;
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};

pub(super) struct Visitor<'a, 'i> {
    expressions: &'i DefaultReferenceExpressionIndexV1,
    receiver: &'a OptionalTemplateReceiverV1,
    result: DefaultSourceReferenceClosureV1<'a>,
    callables: Domain<'a, ExportDefaultCallableTargetV1>,
    constructors: Domain<'a, DefaultConstructorRefV1>,
    types: Domain<'a, SignatureTypeKey>,
    globals: Domain<'a, PersistentPropertyId>,
    singletons: Domain<'a, PersistentObjectValueId>,
    fields: Domain<'a, DefaultFieldRefV1>,
}
impl<'a, 'i> Visitor<'a, 'i> {
    pub fn new(
        template: &'a DefaultSourceTemplateV1,
        expressions: &'i DefaultReferenceExpressionIndexV1,

        path: &WirePath,
    ) -> Result<Self, Error> {
        use ExportDefaultReferenceKindV1 as Kind;
        let refs = template.references();
        // Each source domain is bounded by u32; their sum fits in u64.
        let count: u64 = [
            refs.callables().len(),
            refs.constructors().len(),
            refs.types().len(),
            refs.globals().len(),
            refs.singleton_values().len(),
            refs.fields().len(),
        ]
        .into_iter()
        .map(|length| length as u64)
        .sum();

        let mut occurrences = Vec::new();
        scoop_wire::allocation::try_reserve_count(&mut occurrences, count, path)?;
        Ok(Self {
            expressions,
            receiver: template.receiver(),
            result: DefaultSourceReferenceClosureV1 {
                template: template.key(),
                occurrences,
            },
            callables: Domain::new(refs.callables(), Kind::Callable),
            constructors: Domain::new(refs.constructors(), Kind::Constructor),
            types: Domain::new(refs.types(), Kind::Type),
            globals: Domain::new(refs.globals(), Kind::Global),
            singletons: Domain::new(refs.singleton_values(), Kind::Singleton),
            fields: Domain::new(refs.fields(), Kind::Field),
        })
    }
    pub fn finish(self) -> Result<DefaultSourceReferenceClosureV1<'a>, Error> {
        self.callables.finish()?;
        self.constructors.finish()?;
        self.types.finish()?;
        self.globals.finish()?;
        self.singletons.finish()?;
        self.fields.finish()?;
        Ok(self.result)
    }
}
impl<'a> DefaultBodyReferenceVisitorV1<'a> for Visitor<'a, '_> {
    type Error = Error;
    fn expression(
        &mut self,
        _: u32,
        _: &'a DefaultExpressionV1,

        _: &WirePath,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'a>,

        path: &WirePath,
    ) -> Result<(), Error> {
        use DefaultBodyReferenceTargetV1 as Target;
        let (index, source) = match occurrence.target {
            Target::Callable(target) => {
                let (i, r) = self.callables.observe(
                    occurrence,
                    |declared, p| target.compare_to(declared, p),
                    path,
                )?;
                (i, Record::Callable(r))
            }
            Target::Constructor(target) => {
                let (i, r) = self.constructors.observe(
                    occurrence,
                    |declared, p| target.compare_to(declared, p),
                    path,
                )?;
                (i, Record::Constructor(r))
            }
            Target::Type(target) => {
                let (i, r) = self.types.observe(
                    occurrence,
                    |declared, p| compare_default_signature_reference_targets(target, declared, p),
                    path,
                )?;
                (i, Record::Type(r))
            }
            Target::Global(target) => {
                let (i, r) = self.globals.observe(
                    occurrence,
                    |declared, _| Ok(target.cmp(declared)),
                    path,
                )?;
                (i, Record::Global(r))
            }
            Target::Singleton(target) => {
                let (i, r) = self.singletons.observe(
                    occurrence,
                    |declared, _| Ok(target.cmp(declared)),
                    path,
                )?;
                (i, Record::Singleton(r))
            }
            Target::Field(target) => {
                let (i, r) = self.fields.observe(
                    occurrence,
                    |declared, p| target.compare_to(declared, p),
                    path,
                )?;
                (i, Record::Field(r))
            }
        };
        let context =
            project_default_reference_context(occurrence, self.receiver, self.expressions)
                .map_err(|error| match error {
                    DefaultReferenceReceiverError::Resource(error) => Error::Resource(error),
                    DefaultReferenceReceiverError::ReceiverOutsideBody => {
                        Error::ReceiverOutsideBody
                    }
                })?;
        self.result
            .occurrences
            .push(DefaultSourceReferenceOccurrenceV1 {
                index,
                body: occurrence,
                source,
                context,
            });
        Ok(())
    }
}
