//! Type and finite-shape agreement with the shared HIR declarations.

mod applications;
mod errors;
mod helpers;
mod representation;
mod validation;
use SharedMirTypeValidationError as Error;
pub use errors::{SharedMirTypeComponent, SharedMirTypeValidationError};
pub use validation::validate_shared_mir_type_exports;
