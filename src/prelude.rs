// src/prelude.rs

//! The Ordex Prelude
//!
//! The purpose of this module is to alleviate imports of commonly-used
//! items of the `ordex` crate by adding a glob import to the top of modules:
//!
//! ```
//! use ordex::prelude::*;
//! ```

pub use crate::align;
pub use crate::core::{Index, OrdexArena, VerifiedIndices};
