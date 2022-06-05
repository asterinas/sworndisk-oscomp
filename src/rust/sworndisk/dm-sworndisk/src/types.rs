use crate::{BLOCK_SIZE, SWORNDISK_KEY_LENGTH, SWORNDISK_MAC_LENGTH, SWORNDISK_NONCE_LENGTH};

/// AES-128-GCM key type
pub type KeyType = [u8; SWORNDISK_KEY_LENGTH];

/// AES-128-GCM iv (nonce) type
pub type NonceType = [u8; SWORNDISK_NONCE_LENGTH];

/// AES-128-GCM mac (tag) type
pub type MacType = [u8; SWORNDISK_MAC_LENGTH];

/// Block slice type
pub type BlockType = [u8; BLOCK_SIZE as usize];
