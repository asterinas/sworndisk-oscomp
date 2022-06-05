use crate::prelude::*;
use crate::types::{KeyType, MacType, NonceType};
use crate::utils::*;

/// BIT Record
#[derive(Copy, Clone, Debug)]
pub struct Record {
    /// HBA (Hardware Block Address)
    pub hba: u64,
    /// Crypto key
    pub key: KeyType,
    /// Crypto random string (a.k.a nonce / iv)
    pub nonce: NonceType,
    /// Crypto authentication data (a.k.a MAC / tag)
    pub mac: MacType,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            hba: 0,
            key: [0; SWORNDISK_KEY_LENGTH],
            mac: [0; SWORNDISK_MAC_LENGTH],
            nonce: [0; SWORNDISK_NONCE_LENGTH],
        }
    }
}

/// The size of BIT record
pub const SWORNDISK_RECORD_SIZE: usize = mem::size_of::<Record>();

impl Serialize for Record {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { core::mem::transmute::<u64, [u8; 8]>(self.hba) })?;
        vec.try_extend_from_slice(&self.key)?;
        vec.try_extend_from_slice(&self.nonce)?;
        vec.try_extend_from_slice(&self.mac)?;
        vec.try_resize(SWORNDISK_RECORD_SIZE, 0u8)?;

        Ok(vec)
    }
}

impl Deserialize for Record {
    fn deserialize(buffer: &[u8]) -> Result<Self> {
        if buffer.len() != SWORNDISK_RECORD_SIZE {
            return Err(EINVAL);
        }

        let hba = unsafe { core::mem::transmute::<[u8; 8], u64>(buffer[0..8].try_into().unwrap()) };
        let key_index = 8;
        let key = buffer[key_index..key_index + SWORNDISK_KEY_LENGTH]
            .try_into()
            .unwrap();
        let nonce_index = key_index + SWORNDISK_KEY_LENGTH;
        let nonce = buffer[nonce_index..nonce_index + SWORNDISK_NONCE_LENGTH]
            .try_into()
            .unwrap();
        let mac_index = nonce_index + SWORNDISK_NONCE_LENGTH;
        let mac = buffer[mac_index..mac_index + SWORNDISK_MAC_LENGTH]
            .try_into()
            .unwrap();

        Ok(Self {
            hba,
            key,
            nonce,
            mac,
        })
    }
}
