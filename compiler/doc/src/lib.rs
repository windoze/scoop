//! Documentation syntax and presentation, independent of compiler stages.

mod markdown;
mod normalize;
mod paths;
mod syntax;
mod tags;

pub use syntax::prepare_source;

#[cfg(test)]
mod tests;
