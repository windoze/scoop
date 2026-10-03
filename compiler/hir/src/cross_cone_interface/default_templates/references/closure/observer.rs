use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::WirePath;

use super::compare::{
    CallableTargetView, ConstructorTargetView, FieldTargetView, callable_target,
    constructor_target, field_target, signature_type,
};
use super::visitor::{
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1 as Target,
    DefaultBodyReferenceVisitorV1,
};
use super::{
    ExportDefaultReferenceClosureValidationError, ExportDefaultReferenceKindV1,
    ExportDefaultReferenceOccurrenceSiteV1, ExportDefaultReferenceSetV1, ExportDefinitionSourceV1,
    ReferenceDomains, observe_record,
};

pub(super) struct ClosureObserver<'a> {
    domains: ReferenceDomains<'a>,
}

impl<'a> ClosureObserver<'a> {
    pub(super) fn new(
        references: &'a ExportDefaultReferenceSetV1,

        path: &WirePath,
    ) -> Result<Self, ExportDefaultReferenceClosureValidationError> {
        let domains = ReferenceDomains::new(references, path)
            .map_err(ExportDefaultReferenceClosureValidationError::Resource)?;
        Ok(Self { domains })
    }

    pub(super) fn finish(&self) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.domains.finish()
    }

    fn observe_callable(
        &mut self,
        target: CallableTargetView<'_>,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,

        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.callables.records,
            &mut self.domains.callables.seen,
            origin,
            ExportDefaultReferenceKindV1::Callable,
            site,
            |declared, path| callable_target(declared, target, path),
            path,
        )
    }

    fn observe_constructor(
        &mut self,
        target: ConstructorTargetView<'_>,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,

        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.constructors.records,
            &mut self.domains.constructors.seen,
            origin,
            ExportDefaultReferenceKindV1::Constructor,
            site,
            |declared, path| constructor_target(declared, target, path),
            path,
        )
    }

    fn match_type(
        &mut self,
        target: &SignatureTypeKey,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,

        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        if matches!(target, SignatureTypeKey::Binder { .. }) {
            return Ok(());
        }
        observe_record(
            self.domains.types.records,
            &mut self.domains.types.seen,
            origin,
            ExportDefaultReferenceKindV1::Type,
            site,
            |declared, path| signature_type(declared, target, path),
            path,
        )
    }

    fn observe_global(
        &mut self,
        target: PersistentPropertyId,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,

        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.globals.records,
            &mut self.domains.globals.seen,
            origin,
            ExportDefaultReferenceKindV1::Global,
            site,
            |declared, _| Ok(declared.cmp(&target)),
            path,
        )
    }

    fn observe_singleton(
        &mut self,
        target: PersistentObjectValueId,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,

        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.singletons.records,
            &mut self.domains.singletons.seen,
            origin,
            ExportDefaultReferenceKindV1::Singleton,
            site,
            |declared, _| Ok(declared.cmp(&target)),
            path,
        )
    }

    fn observe_field(
        &mut self,
        target: FieldTargetView<'_>,
        origin: &ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,

        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        observe_record(
            self.domains.fields.records,
            &mut self.domains.fields.seen,
            origin,
            ExportDefaultReferenceKindV1::Field,
            site,
            |declared, path| field_target(declared, target, path),
            path,
        )
    }
}

impl<'body> DefaultBodyReferenceVisitorV1<'body> for ClosureObserver<'_> {
    type Error = ExportDefaultReferenceClosureValidationError;

    fn expression(
        &mut self,
        _: u32,
        _: &crate::DefaultExpressionV1,

        _: &WirePath,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,

        path: &WirePath,
    ) -> Result<(), Self::Error> {
        let DefaultBodyReferenceOccurrenceV1 {
            target,
            definition_origin: origin,
            site,
            ..
        } = occurrence;
        match target {
            Target::Callable(target) => self.observe_callable(target, origin, site, path),
            Target::Constructor(target) => self.observe_constructor(target, origin, site, path),
            Target::Type(target) => self.match_type(target, origin, site, path),
            Target::Global(target) => self.observe_global(target, origin, site, path),
            Target::GenericDelegate(target) => Err(
                ExportDefaultReferenceClosureValidationError::DirectGenericDelegate {
                    property: target.property(),
                    site,
                },
            ),
            Target::Singleton(target) => self.observe_singleton(target, origin, site, path),
            Target::Field(target) => self.observe_field(target, origin, site, path),
        }
    }
}
