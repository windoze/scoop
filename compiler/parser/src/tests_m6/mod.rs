//! Unit tests for the M6 syntax: class declarations (constructor
//! properties, base-class delegation, interfaces, member functions),
//! interface declarations, `open` / `abstract` / `override` modifiers,
//! struct/enum interface lists and member functions, field assignment,
//! `this`, method calls, the type operators `is` / `!is` / `as` / `as?`,
//! reference equality (`===` / `!==`), and every M6 "not supported"
//! diagnostic.

mod class_diagnostics;
mod classes;
mod field_assignment;
mod interfaces;
mod method_calls;
mod reference_equality;
mod type_operators;
mod value_members;
