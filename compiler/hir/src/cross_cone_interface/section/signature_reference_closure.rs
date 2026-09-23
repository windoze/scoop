use std::fmt;

use scoop_identity::{NominalDeclarationOwner, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{CrossConeHirInterfaceSectionV1, signature_nominal_walk::SignatureNominalWalker};
use crate::{
    CanonicalBinderListV1, CanonicalExternalHirReferencesV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirTargetV1, NominalSourceShapeV1,
    TypeParameterBoundLocation, TypeParameterBoundsV1,
};

impl CrossConeHirInterfaceSectionV1 {
    /// Validates that `SignatureDependency` is exactly the set of foreign
    /// nominal leaves used by declaration and source-call signatures in
    /// fields 2, 3, 4, and 6.
    pub fn validate_signature_reference_closure<A, E>(
        &self,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut validator = SignatureReferenceClosureValidator::new(
            self.external_references(),
            authority,
            meter,
            &path.clone().field(10),
        )?;

        self.visit_nominal_signatures(&mut validator, path)?;
        self.visit_callable_signatures(&mut validator, path)?;
        self.visit_property_signatures(&mut validator, path)?;
        self.visit_source_call_signatures(&mut validator, path)?;

        validator.finish(&path.clone().field(10))
    }

    fn visit_nominal_signatures<A, E>(
        &self,
        validator: &mut SignatureReferenceClosureValidator<'_, '_, A>,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let table_path = path.clone().field(2);
        for (wire_index, (record_index, record)) in
            (0_u64..).zip(self.nominal_interfaces().records().iter().enumerate())
        {
            let record_path = table_path.clone().index(wire_index);
            visit_binder_signatures(
                validator,
                record.type_parameters(),
                &record_path.clone().field(3),
                |binder_index, bound| ExternalHirSignatureUseSiteV1::NominalTypeParameter {
                    record_index,
                    binder_index,
                    bound,
                },
            )?;

            for (signature_index, signature) in
                record.exact_supertypes().values().iter().enumerate()
            {
                validator.visit_signature(
                    signature,
                    ExternalHirSignatureUseSiteV1::NominalSupertype {
                        record_index,
                        signature_index,
                    },
                    &record_path.clone().field(4).index(signature_index as u64),
                )?;
            }

            for (field_index, field) in record.source_shape().declared_fields().iter().enumerate() {
                validator.visit_signature(
                    field.value_type(),
                    ExternalHirSignatureUseSiteV1::NominalField {
                        record_index,
                        field_index,
                    },
                    &record_path
                        .clone()
                        .field(8)
                        .field(record.source_shape().declared_fields_wire_field())
                        .index(field_index as u64)
                        .field(2),
                )?;
            }
            match record.source_shape() {
                NominalSourceShapeV1::Enum(shape) => {
                    for (variant_index, variant) in shape.variants().iter().enumerate() {
                        for (field_index, field) in variant.fields().iter().enumerate() {
                            validator.visit_signature(
                                field.value_type(),
                                ExternalHirSignatureUseSiteV1::NominalEnumField {
                                    record_index,
                                    variant_index,
                                    field_index,
                                },
                                &record_path
                                    .clone()
                                    .field(8)
                                    .field(1)
                                    .index(variant_index as u64)
                                    .field(3)
                                    .index(field_index as u64)
                                    .field(2),
                            )?;
                        }
                    }
                }
                NominalSourceShapeV1::Struct(_)
                | NominalSourceShapeV1::Class(_)
                | NominalSourceShapeV1::Interface
                | NominalSourceShapeV1::Object(_)
                | NominalSourceShapeV1::Intrinsic(_) => continue,
            }
        }
        Ok(())
    }

    fn visit_callable_signatures<A, E>(
        &self,
        validator: &mut SignatureReferenceClosureValidator<'_, '_, A>,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let table_path = path.clone().field(3);
        for (wire_index, (record_index, record)) in
            (0_u64..).zip(self.callable_interfaces().records().iter().enumerate())
        {
            let record_path = table_path.clone().index(wire_index);
            visit_binder_signatures(
                validator,
                record.type_parameters(),
                &record_path.clone().field(3),
                |binder_index, bound| ExternalHirSignatureUseSiteV1::CallableTypeParameter {
                    record_index,
                    binder_index,
                    bound,
                },
            )?;
            if let Some(receiver) = record.receiver() {
                validator.visit_signature(
                    receiver,
                    ExternalHirSignatureUseSiteV1::CallableReceiver { record_index },
                    &record_path.clone().field(4).field(1),
                )?;
            }
            for (parameter_index, parameter) in record.parameters().parameters().iter().enumerate()
            {
                validator.visit_signature(
                    parameter.value_type(),
                    ExternalHirSignatureUseSiteV1::CallableParameter {
                        record_index,
                        parameter_index,
                    },
                    &record_path
                        .clone()
                        .field(5)
                        .index(parameter_index as u64)
                        .field(2),
                )?;
            }
            validator.visit_signature(
                record.result(),
                ExternalHirSignatureUseSiteV1::CallableResult { record_index },
                &record_path.clone().field(6),
            )?;
        }
        Ok(())
    }

    fn visit_property_signatures<A, E>(
        &self,
        validator: &mut SignatureReferenceClosureValidator<'_, '_, A>,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let table_path = path.clone().field(4);
        for (wire_index, (record_index, record)) in
            (0_u64..).zip(self.property_interfaces().records().iter().enumerate())
        {
            let record_path = table_path.clone().index(wire_index);
            visit_binder_signatures(
                validator,
                record.type_parameters(),
                &record_path.clone().field(3),
                |binder_index, bound| ExternalHirSignatureUseSiteV1::PropertyTypeParameter {
                    record_index,
                    binder_index,
                    bound,
                },
            )?;
            if let Some(receiver) = record.receiver() {
                validator.visit_signature(
                    receiver,
                    ExternalHirSignatureUseSiteV1::PropertyReceiver { record_index },
                    &record_path.clone().field(4).field(1),
                )?;
            }
            validator.visit_signature(
                record.value_type(),
                ExternalHirSignatureUseSiteV1::PropertyValue { record_index },
                &record_path.clone().field(5),
            )?;
        }
        Ok(())
    }

    fn visit_source_call_signatures<A, E>(
        &self,
        validator: &mut SignatureReferenceClosureValidator<'_, '_, A>,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let table_path = path.clone().field(6);
        for (wire_index, (record_index, record)) in
            (0_u64..).zip(self.source_interfaces().records().iter().enumerate())
        {
            let record_path = table_path.clone().index(wire_index);
            for (parameter_index, parameter) in record.parameters().parameters().iter().enumerate()
            {
                let parameter_path = record_path.clone().field(2).index(parameter_index as u64);
                validator.visit_signature(
                    parameter.value_type(),
                    ExternalHirSignatureUseSiteV1::SourceParameter {
                        record_index,
                        parameter_index,
                    },
                    &parameter_path.clone().field(2),
                )?;
                if let Some(element_type) = parameter.calling().element_type() {
                    validator.visit_signature(
                        element_type,
                        ExternalHirSignatureUseSiteV1::SourceVarargElement {
                            record_index,
                            parameter_index,
                        },
                        &parameter_path.clone().field(3).field(1),
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn visit_binder_signatures<A, E, F>(
    validator: &mut SignatureReferenceClosureValidator<'_, '_, A>,
    binders: &CanonicalBinderListV1,
    path: &WirePath,
    site: F,
) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
where
    A: ExternalHirReferenceSemanticAuthority<E>,
    F: Fn(usize, TypeParameterBoundLocation) -> ExternalHirSignatureUseSiteV1,
{
    for (wire_index, (binder_index, binder)) in (0_u64..).zip(binders.binders().iter().enumerate())
    {
        let TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() else {
            continue;
        };
        let bounds_path = path.clone().index(wire_index).field(2);
        if let Some(class) = bounds.class() {
            let bound = TypeParameterBoundLocation::Class;
            validator.visit_signature(
                class,
                site(binder_index, bound),
                &bounds_path.clone().field(1).field(1),
            )?;
        }
        for (interface_index, interface) in bounds.interfaces().values().iter().enumerate() {
            let bound = TypeParameterBoundLocation::Interface { interface_index };
            validator.visit_signature(
                interface,
                site(binder_index, bound),
                &bounds_path.clone().field(2).index(interface_index as u64),
            )?;
        }
    }
    Ok(())
}

struct SignatureReferenceClosureValidator<'references, 'validation, A> {
    references: &'references CanonicalExternalHirReferencesV1,
    seen: Vec<bool>,
    current: scoop_identity::ConeIdentity,
    authority: &'validation mut A,
    meter: &'validation mut BudgetMeter,
}

impl<'references, 'validation, A> SignatureReferenceClosureValidator<'references, 'validation, A> {
    fn new<E>(
        references: &'references CanonicalExternalHirReferencesV1,
        authority: &'validation mut A,
        meter: &'validation mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut seen = Vec::new();
        meter
            .try_reserve_collection_slots(&mut seen, references.records().len(), path)
            .map_err(ExternalHirSignatureClosureValidationError::Resource)?;
        seen.resize(references.records().len(), false);
        Ok(Self {
            references,
            seen,
            current: authority.current_cone(),
            authority,
            meter,
        })
    }

    fn visit_signature<E>(
        &mut self,
        signature: &SignatureTypeKey,
        site: ExternalHirSignatureUseSiteV1,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut walker = SignatureNominalWalker::new(signature, self.meter, path)
            .map_err(ExternalHirSignatureClosureValidationError::Resource)?;
        while let Some(declaration) = walker
            .next(self.meter, path)
            .map_err(ExternalHirSignatureClosureValidationError::Resource)?
        {
            self.observe_nominal(declaration, site, path)?;
        }
        Ok(())
    }

    fn observe_nominal<E>(
        &mut self,
        declaration: NominalDeclarationOwner,
        site: ExternalHirSignatureUseSiteV1,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let target = ExternalHirTargetV1::from(declaration);
        let expected = self
            .authority
            .external_hir_target_origin(target)
            .map_err(
                |error| ExternalHirSignatureClosureValidationError::TargetOrigin {
                    site,
                    target,
                    error,
                },
            )?;
        if expected == self.current {
            return Ok(());
        }

        let record_index = self
            .references
            .find_index_metered(target, self.meter, path)
            .map_err(ExternalHirSignatureClosureValidationError::Resource)?
            .ok_or(ExternalHirSignatureClosureValidationError::MissingReference { site, target })?;
        let record = &self.references.records()[record_index];
        if record.origin() != expected {
            return Err(ExternalHirSignatureClosureValidationError::OriginMismatch(
                Box::new(ExternalHirSignatureOriginMismatch {
                    site,
                    record_index,
                    target,
                    expected,
                    actual: record.origin(),
                }),
            ));
        }
        if !record
            .roles()
            .contains(ExternalHirReferenceRoleV1::SignatureDependency)
        {
            return Err(ExternalHirSignatureClosureValidationError::MissingRole {
                site,
                record_index,
                target,
            });
        }
        self.seen[record_index] = true;
        Ok(())
    }

    fn finish<E>(
        self,
        path: &WirePath,
    ) -> Result<(), ExternalHirSignatureClosureValidationError<E>> {
        for (record_index, record) in self.references.records().iter().enumerate() {
            self.meter
                .charge_work(1, path)
                .map_err(ExternalHirSignatureClosureValidationError::Resource)?;
            if record
                .roles()
                .contains(ExternalHirReferenceRoleV1::SignatureDependency)
                && !self.seen[record_index]
            {
                return Err(ExternalHirSignatureClosureValidationError::ExtraRole {
                    record_index,
                    target: record.target(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHirSignatureUseSiteV1 {
    NominalTypeParameter {
        record_index: usize,
        binder_index: usize,
        bound: TypeParameterBoundLocation,
    },
    NominalSupertype {
        record_index: usize,
        signature_index: usize,
    },
    NominalField {
        record_index: usize,
        field_index: usize,
    },
    NominalEnumField {
        record_index: usize,
        variant_index: usize,
        field_index: usize,
    },
    CallableTypeParameter {
        record_index: usize,
        binder_index: usize,
        bound: TypeParameterBoundLocation,
    },
    CallableReceiver {
        record_index: usize,
    },
    CallableParameter {
        record_index: usize,
        parameter_index: usize,
    },
    CallableResult {
        record_index: usize,
    },
    PropertyTypeParameter {
        record_index: usize,
        binder_index: usize,
        bound: TypeParameterBoundLocation,
    },
    PropertyReceiver {
        record_index: usize,
    },
    PropertyValue {
        record_index: usize,
    },
    SourceParameter {
        record_index: usize,
        parameter_index: usize,
    },
    SourceVarargElement {
        record_index: usize,
        parameter_index: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExternalHirSignatureClosureValidationError<E> {
    TargetOrigin {
        site: ExternalHirSignatureUseSiteV1,
        target: ExternalHirTargetV1,
        error: E,
    },
    MissingReference {
        site: ExternalHirSignatureUseSiteV1,
        target: ExternalHirTargetV1,
    },
    OriginMismatch(Box<ExternalHirSignatureOriginMismatch>),
    MissingRole {
        site: ExternalHirSignatureUseSiteV1,
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    ExtraRole {
        record_index: usize,
        target: ExternalHirTargetV1,
    },
    Resource(WireError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalHirSignatureOriginMismatch {
    pub site: ExternalHirSignatureUseSiteV1,
    pub record_index: usize,
    pub target: ExternalHirTargetV1,
    pub expected: scoop_identity::ConeIdentity,
    pub actual: scoop_identity::ConeIdentity,
}

impl<E: fmt::Display> fmt::Display for ExternalHirSignatureClosureValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetOrigin {
                site,
                target,
                error,
            } => write!(
                formatter,
                "signature target {target:?} used at {site:?} has no canonical origin: {error}"
            ),
            Self::MissingReference { site, target } => write!(
                formatter,
                "foreign signature target {target:?} used at {site:?} is absent from the external HIR reference table"
            ),
            Self::OriginMismatch(error) => write!(
                formatter,
                "signature target {:?} used at {:?} uses external record {} with origin {}, expected {}",
                error.target, error.site, error.record_index, error.actual, error.expected
            ),
            Self::MissingRole {
                site,
                record_index,
                target,
            } => write!(
                formatter,
                "signature target {target:?} used at {site:?} uses external record {record_index} without SignatureDependency role"
            ),
            Self::ExtraRole {
                record_index,
                target,
            } => write!(
                formatter,
                "external record {record_index} for target {target:?} has SignatureDependency role without a signature use in fields 2, 3, 4, or 6"
            ),
            Self::Resource(error) => write!(
                formatter,
                "external signature reference closure resource failure: {error}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExternalHirSignatureClosureValidationError<E>
{
}

#[cfg(test)]
mod tests;
