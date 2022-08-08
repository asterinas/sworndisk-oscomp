#![feature(const_fn_trait_bound)]

//! Async task (work queue) ability

extern crate alloc;
extern crate kernel;

const __LOG_PREFIX: &[u8] = b"async-work\0";

mod prelude;
mod work_queue;

pub use work_queue::*;
