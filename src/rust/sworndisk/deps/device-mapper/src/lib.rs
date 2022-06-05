//! Rust wrapper of Linux device mapper API

#![feature(const_fn_trait_bound)]

extern crate alloc;
extern crate kernel;

const __LOG_PREFIX: &[u8] = b"rust-dm\0";

mod block;
mod block_manager;
mod block_validator;
mod callbacks;
mod consts;
mod io;
mod prelude;
mod wrappers;

pub mod macros;
pub mod utils;

pub use block::*;
pub use block_manager::*;
pub use block_validator::*;
pub use callbacks::*;
pub use consts::*;
pub use io::*;
pub use wrappers::*;
