//! Class / interface resolution and inheritance checks (M6,
//! milestone6 DESIGN.md 2.2).
//!
//! Pass 2 (`resolve_class`): constructor properties (duplicate names
//! diagnosed), the base-class clause (the base must be an `open` or
//! `abstract` class) and the interface list.
//!
//! Pass 2.5 (`resolve_method_signature`): member signatures. A generic
//! host's parameters form the prefix of the method's parameter space and
//! parameters declared by the method form the suffix. Generic member
//! functions are static-only (spec 3.2). Bodyless declarations — interface
//! methods and `abstract`
//! class methods — get their parameter-only body here (`this` plus
//! the declared parameters; the statements are empty because there is
//! nothing to execute — MIR only ever reaches them through a vtable /
//! itable slot that names a concrete override).
//!
//! Pass 2.75 (`check_inheritance`): inheritance cycles, property
//! shadowing (M6 simplification: a property may not reuse a base
//! property's name), the `override` rules (overriding without the
//! modifier and the modifier without an overridden method are both
//! diagnostics; implementations of interface methods require it,
//! DESIGN.md 5.2) and interface implementation (every interface
//! method must have a same-signature concrete method on the class or
//! its base chain — name, parameter types and return type all equal;
//! overloads only match exactly, M7).
//!
//! Object layout decision (see the crate docs): a subclass object is
//! laid out as the base class's fields followed by its own, with
//! consecutive indices — `FieldRef::ClassField::index` is the absolute
//! layout index and `class_id` the class that declared the property.
//! This is orthogonal to the vtable layout (mir-lower's job).

use scoop_ast as ast;
use scoop_hir as hir;

use hir::{ClassId, FunctionId, Type, TypeId};

use crate::{FnParam, FnSig, ForbiddenSuspendContext, Lowerer, Owner, SuspensionContext};

mod construction;
mod declarations;
mod hierarchy;
mod inheritance;
mod lookup;
mod methods;
mod variance;
