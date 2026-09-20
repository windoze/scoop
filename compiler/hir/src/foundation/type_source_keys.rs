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
}
