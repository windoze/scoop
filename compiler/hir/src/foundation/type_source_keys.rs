use super::*;

impl CanonicalHirFoundation {
    pub(crate) fn type_source_exact_records(&self) -> &[ExactTypeRecord] {
        &self.exact_types
    }

    pub(crate) fn type_source_nominal_records(&self) -> &[TypeRecord] {
        &self.types
    }

    pub(crate) fn type_source_generic_records(&self) -> &[GenericTypeRecord] {
        &self.generic_types
    }

    pub(crate) fn type_source_generated_records(&self) -> &[GeneratedTypeRecord] {
        &self.generated_types
    }

    pub(crate) fn type_source_accessor_records(&self) -> &[PropertyAccessorRecord] {
        &self.property_accessors
    }
    pub(crate) fn type_source_function_records(&self) -> &[FunctionRecord] {
        &self.functions
    }

    pub(crate) fn type_source_property_records(&self) -> &[PropertyRecord] {
        &self.properties
    }

    pub(crate) fn type_source_dispatch_records(&self) -> &[DispatchSlotRecord] {
        &self.dispatch_slots
    }
}
