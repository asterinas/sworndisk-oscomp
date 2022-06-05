//! Rust wrapper of Linux Crypto API

// extern crate alloc;
extern crate kernel;

const __LOG_PREFIX: &[u8] = b"crypto\0";

mod prelude;

pub mod aead;
pub mod aead_request;
pub mod scatter_list;

use prelude::*;

pub use aead::*;
pub use aead_request::*;
pub use scatter_list::*;

/// Generate N random bytes
pub fn get_random_bytes(nbytes: usize) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::new();
    buf.try_resize(nbytes, 0)?;

    // SAFETY: Safe. `buf` has n bytes long.
    unsafe { bindings::get_random_bytes_arch(buf.as_mut_ptr() as *mut c_void, nbytes as i32) };

    Ok(buf)
}
