//! # Mulligan strategies
//!
//! The `mulligan` module defines a `Mulligan` trait and
//! several implementations of different mulligan strategies.

mod london;
mod mulligan;
mod never;
mod vancouver;

pub use london::London;
pub use mulligan::Mulligan;
pub use never::Never;
pub use vancouver::Vancouver;
