//! Current-target test declarations checked by the frontend.

use crate::{ClassConstructorId, ClassId, DefinitionOrigin, FunctionId};

/// One direct source method used by a case or lifecycle hook.
#[derive(Clone, Debug)]
pub struct TestMethod {
    pub function: FunctionId,
    pub name: String,
    pub origin: DefinitionOrigin,
}

#[derive(Clone, Debug)]
pub struct TestCase {
    pub method: TestMethod,
    pub display_name: String,
    pub ignore_reason: Option<String>,
}

/// A non-generic concrete class with a uniquely selected safe constructor.
/// Defaults are evaluated by the generated invocation, never by discovery.
#[derive(Clone, Debug)]
pub struct TestContainer {
    pub class: ClassId,
    pub constructor: ClassConstructorId,
    pub qualified_name: String,
    pub origin: DefinitionOrigin,
    pub before: Option<TestMethod>,
    pub after: Option<TestMethod>,
    pub cases: Vec<TestCase>,
}
