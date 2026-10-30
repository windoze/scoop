//! Representative values for LIR unit tests. Production targets live in
//! scoop-target-data; target catalog tests run in that crate.

mod backend;
pub use backend::*;

mod selection;
pub use selection::*;

mod layout;
pub use layout::*;

mod c_bridge;
pub use c_bridge::*;

mod ids;
pub use ids::*;
