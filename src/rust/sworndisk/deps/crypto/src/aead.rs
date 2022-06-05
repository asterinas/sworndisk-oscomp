//! Authenticated Encryption With Associated Data (AEAD) Cipher API

use core::{fmt, fmt::Debug};

use kernel::sync::SpinLock;

use crate::{aead_request::AeadRequest, prelude::*, scatter_list::ScatterList};

/// ScatterList length to make a encrypt / decrypt request
///
/// The data distributon in ScatterList should be like:
///
/// | assoc-data | plain | --- encrypt --> | assoc-data | cipher | mac |
///                        <-- decrypt ---
const AES_GCM_SCATTER_LIST_LEN: usize = 3;

/// length of AES-GCM Tag (or, MAC)
const AES_GCM_TAG_LEN: usize = 16;

/// Authenticated Encryption With Associated Data
pub struct Aead {
    inner: SpinLock<*mut bindings::crypto_aead>,
}

impl Debug for Aead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Aead").finish()
    }
}

impl Aead {
    /// Create a new AEAD crypto handle
    pub fn new(algorithm: &'static CStr, _type: u32, mask: u32) -> Result<Pin<Box<Self>>> {
        // Alloc a new crypto handle and check it is valid.
        // SAFETY: Calling FFI function
        let crypto_aead = unsafe {
            let crypto_aead = bindings::crypto_alloc_aead(algorithm.as_char_ptr(), _type, mask);

            if bindings::IS_ERR(crypto_aead as *const c_void) {
                // Since the `crypto_aead` is error, the unwrap_err() will not failed
                Err(
                    to_result(|| bindings::PTR_ERR(crypto_aead as *const c_void) as i32)
                        .unwrap_err(),
                )
            } else {
                Ok(crypto_aead)
            }
        }?;

        // Create a SpinLock to protect the inner crypto_aead.
        // Since the pointer is created by Aead::new(), we can guarantee the safety by protecting it with a SpinLock.
        let mut aead = Pin::from(Box::try_new(Self {
            // SAFETY: Safe, SpinLock is initialized in the call to `spinlock_init` below.
            inner: unsafe { SpinLock::new(crypto_aead) },
        })?);
        let inner = unsafe { aead.as_mut().map_unchecked_mut(|t| &mut t.inner) };

        kernel::spinlock_init!(inner, "Aead::inner");

        Ok(aead)
    }

    /// Set key for cipher
    fn set_key(ptr: *mut bindings::crypto_aead, key: &Vec<u8>) -> Result {
        let len = key.len() as u32;
        let key = key.as_ptr();

        // SAFETY: Calling FFI function
        to_result(|| unsafe { bindings::crypto_aead_setkey(ptr, key, len) })
    }

    /// Get the raw pointer reference of `struct crypto_aead`
    pub fn raw(&self) -> *mut bindings::crypto_aead {
        *self.inner.lock()
    }

    /// encrypt the data with key, nonce, plain-text, returns (cipher-text, mac)
    pub fn encrypt(
        self: Pin<&Self>,
        key: &Vec<u8>,
        nonce: &mut Vec<u8>,
        plain: &mut Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        // SAFETY: Safe, `self.inner` is still pinned.
        let inner_lock = unsafe { self.as_ref().map_unchecked(|t| &t.inner) };
        let inner = inner_lock.lock();
        let plain_len = plain.len();

        // set key
        Self::set_key(*inner, key)?;

        // allocate buffer for store encrypt result
        let mut cipher = Vec::try_with_capacity(plain_len)?;
        let mut mac = Vec::try_with_capacity(AES_GCM_TAG_LEN)?;

        cipher.try_resize(plain_len, 0)?;
        mac.try_resize(AES_GCM_TAG_LEN, 0)?;

        // initialize AEAD request struct and ScatterList
        // SAFETY: Safe. `inner` is valid and lock-guarded.
        let mut req = unsafe { AeadRequest::new(*inner)? };
        let mut sg_in = ScatterList::<{ AES_GCM_SCATTER_LIST_LEN }>::new()?;
        let mut sg_out = ScatterList::<{ AES_GCM_SCATTER_LIST_LEN }>::new()?;

        sg_in.set_buf(0, plain, plain_len)?;

        sg_out.set_buf(0, &mut cipher, plain_len)?;
        sg_out.set_buf(1, &mut mac, AES_GCM_TAG_LEN)?;

        // set encrypt payload and submit encrypt request
        req.set_crypt::<AES_GCM_SCATTER_LIST_LEN, AES_GCM_SCATTER_LIST_LEN>(
            &mut sg_in,
            &mut sg_out,
            plain_len,
            nonce,
        )?;
        req.set_assoc_data_len(0)?;
        req.encrypt()?;

        Ok((cipher, mac))
    }

    /// Encrypt the data with key, nonce, plain-text.
    ///
    /// This method will encrypt the data in its place (i.e. plain -> cipher) and returns MAC.
    ///
    /// This method is marked as unsafe because it violates the mutability rules of Rust.
    pub unsafe fn encrypt_in_place(
        self: Pin<&Self>,
        key: &Vec<u8>,
        nonce: &mut Vec<u8>,
        plain: &mut [u8],
        len: usize,
    ) -> Result<Vec<u8>> {
        // SAFETY: Safe, `self.inner` is still pinned.
        let inner_lock = unsafe { self.as_ref().map_unchecked(|t| &t.inner) };
        let inner = inner_lock.lock();
        let plain_len = len;

        // set key
        Self::set_key(*inner, key)?;

        // allocate buffer for store encrypt result
        let mut mac = Vec::try_with_capacity(AES_GCM_TAG_LEN)?;
        mac.try_resize(AES_GCM_TAG_LEN, 0)?;

        let mut req = unsafe { AeadRequest::new(*inner)? };
        let mut sg = ScatterList::<{ AES_GCM_SCATTER_LIST_LEN }>::new()?;
        sg.set_buf_slice(0, plain, plain_len)?;
        sg.set_buf(1, &mut mac, AES_GCM_TAG_LEN)?;

        let sg_ptr = sg.raw();
        req.set_assoc_data_len(0)?;
        unsafe {
            bindings::aead_request_set_crypt(
                req.raw(),
                core::mem::transmute(sg_ptr),
                core::mem::transmute(sg_ptr),
                plain_len as u32,
                nonce.as_mut_ptr(),
            );
        };
        req.encrypt()?;

        Ok(mac)
    }

    /// decrypt the data with key, nonce, cipher-text and mac, returns plain-text
    pub fn decrypt(
        self: Pin<&Self>,
        key: &Vec<u8>,
        mac: &mut Vec<u8>,
        nonce: &mut Vec<u8>,
        cipher: &mut Vec<u8>,
    ) -> Result<Vec<u8>> {
        // SAFETY: Safe, `self.inner` is still pinned.
        let inner_lock = unsafe { self.as_ref().map_unchecked(|t| &t.inner) };
        let inner = inner_lock.lock();

        let cipher_len = cipher.len();
        let mac_len = mac.len();

        // set key
        Self::set_key(*inner, key)?;

        // allocate buffer for store decrypt result
        let mut plain = Vec::try_with_capacity(cipher_len)?;
        plain.try_resize(cipher_len, 0)?;

        // SAFETY: Safe. `inner` is valid and lock-guarded.
        let mut req = unsafe { AeadRequest::new(*inner)? };
        let mut sg_in = ScatterList::<{ AES_GCM_SCATTER_LIST_LEN }>::new()?;
        let mut sg_out = ScatterList::<{ AES_GCM_SCATTER_LIST_LEN }>::new()?;

        sg_in.set_buf(0, cipher, cipher_len)?;
        sg_in.set_buf(1, mac, mac_len)?;

        sg_out.set_buf(0, &mut plain, cipher_len)?;

        req.set_crypt::<AES_GCM_SCATTER_LIST_LEN, AES_GCM_SCATTER_LIST_LEN>(
            &mut sg_in,
            &mut sg_out,
            cipher_len + mac_len,
            nonce,
        )?;
        req.set_assoc_data_len(0)?;
        req.decrypt()?;

        Ok(plain)
    }

    /// Decrypt the data with key, nonce, mac and cipher-text.
    ///
    /// This method will decrypt the data in its place (i.e. cipher -> plain).
    ///
    /// This method is marked as unsafe because it violates the mutability rules of Rust.
    pub unsafe fn decrypt_in_place(
        self: Pin<&Self>,
        key: &Vec<u8>,
        mac: &mut Vec<u8>,
        nonce: &mut Vec<u8>,
        cipher: &mut [u8],
        len: usize,
    ) -> Result {
        // SAFETY: Safe, `self.inner` is still pinned.
        let inner_lock = unsafe { self.as_ref().map_unchecked(|t| &t.inner) };
        let inner = inner_lock.lock();

        let cipher_len = len;
        let mac_len = mac.len();

        // set key
        Self::set_key(*inner, key)?;

        let mut req = unsafe { AeadRequest::new(*inner)? };
        let mut sg = ScatterList::<{ AES_GCM_SCATTER_LIST_LEN }>::new()?;
        sg.set_buf_slice(0, cipher, cipher_len)?;
        sg.set_buf(1, mac, AES_GCM_TAG_LEN)?;

        let sg_ptr = sg.raw();
        req.set_assoc_data_len(0)?;
        unsafe {
            bindings::aead_request_set_crypt(
                req.raw(),
                core::mem::transmute(sg_ptr),
                core::mem::transmute(sg_ptr),
                (cipher_len + mac_len) as u32,
                nonce.as_mut_ptr(),
            );
        };

        req.set_assoc_data_len(0)?;
        req.decrypt()?;

        Ok(())
    }
}

impl Drop for Aead {
    fn drop(&mut self) {
        // SAFETY: `self.inner` is allocated by the `AEAD::new`, thus calling
        // `crypto_free_aead()` to drop it is safe.
        unsafe { bindings::crypto_free_aead(*self.inner.lock()) };
    }
}
