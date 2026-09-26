use super::*;

impl CanonicalHirFoundation {
    pub(crate) fn type_source_generated_callable_records(&self) -> &[GeneratedCallableRecord] {
        &self.generated_callables
    }

    pub(crate) fn type_source_field_records(&self) -> &[FieldRecord] {
        &self.fields
    }

    pub(crate) fn type_source_enum_variant_records(&self) -> &[EnumVariantRecord] {
        &self.enum_variants
    }

    pub(crate) fn type_source_object_value_records(&self) -> &[ObjectValueRecord] {
        &self.object_values
    }

    pub(crate) fn type_source_constructor_records(&self) -> &[ConstructorRecord] {
        &self.constructors
    }

    #[cfg(test)]
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
    pub(crate) fn type_source_generic_function_records(&self) -> &[GenericFunctionRecord] {
        &self.generic_functions
    }

    pub(crate) fn type_source_function_records(&self) -> &[FunctionRecord] {
        &self.functions
    }

    pub(crate) fn type_source_property_records(&self) -> &[PropertyRecord] {
        &self.properties
    }

    pub(crate) fn type_source_extension_property_records(&self) -> &[ExtensionPropertyRecord] {
        &self.extension_properties
    }

    pub(crate) fn type_source_initialization_records(&self) -> &[InitializationUnitRecord] {
        &self.initialization_units
    }

    pub(crate) fn type_source_dispatch_records(&self) -> &[DispatchSlotRecord] {
        &self.dispatch_slots
    }
}
