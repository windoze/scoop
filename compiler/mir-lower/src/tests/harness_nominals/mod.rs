mod applications;
mod classes;
mod identities;
mod interfaces;
mod methods;
mod structs;

pub(super) use identities::{
    test_constructor_identities, test_nominal_identities, test_nominal_identities_without_objects,
    test_property_accessor_identities, test_property_identities,
};
