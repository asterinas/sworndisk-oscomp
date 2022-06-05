//! Rust wrapper for AEAD Request

use crate::{prelude::*, scatter_list::ScatterList};

/// AEAD crypto request
pub struct AeadRequest {
    inner: *mut bindings::aead_request,
}

impl AeadRequest {
    /// Create a new AEAD request
    ///
    /// # Safety
    ///
    /// The caller should guarantee that `aead` is valid and no other thread
    /// is taking the ownership of `aead`.
    pub unsafe fn new(aead: *mut bindings::crypto_aead) -> Result<Self> {
        if aead.is_null() {
            return Err(EINVAL);
        }

        // SAFETY: Calling FFI function
        let inner = unsafe { bindings::aead_request_alloc(aead, bindings::BINDINGS_GFP_KERNEL) };
        match inner.is_null() {
            true => Err(ENOMEM),
            false => Ok(Self { inner }),
        }
    }

    /// Get the raw pointer reference of `self.inner`
    pub fn raw(&self) -> *mut bindings::aead_request {
        self.inner
    }

    /// Set data buffers to encrypt / decrypt
    pub fn set_crypt<const N: usize, const M: usize>(
        &self,
        src: &mut ScatterList<{ N }>,
        dst: &mut ScatterList<{ M }>,
        cryptlen: usize,
        nonce: &mut Vec<u8>,
    ) -> Result {
        unsafe {
            // SAFETY: Calling FFI function
            bindings::aead_request_set_crypt(
                self.inner,
                src.raw_mut(),
                dst.raw_mut(),
                cryptlen as u32,
                nonce.as_mut_ptr(),
            );
        };

        Ok(())
    }

    /// Set the associated data length for AEAD request
    pub fn set_assoc_data_len(&self, assoc_len: usize) -> Result {
        // SAFETY: Calling FFI function
        unsafe {
            bindings::aead_request_set_ad(self.inner, assoc_len as u32);
        };

        Ok(())
    }

    /// Encrypt ciphertext
    pub fn encrypt(&mut self) -> Result {
        to_result(|| unsafe { bindings::crypto_aead_encrypt(self.inner) })
    }

    /// Decrypt ciphertext
    pub fn decrypt(&mut self) -> Result {
        to_result(|| unsafe { bindings::crypto_aead_decrypt(self.inner) })
    }
}

impl Drop for AeadRequest {
    fn drop(&mut self) {
        // SAFETY: Safe. `self.inner` is allocated in `AeadRequest::new` and is non-null.
        unsafe { bindings::aead_request_free(self.inner) };
    }
}
