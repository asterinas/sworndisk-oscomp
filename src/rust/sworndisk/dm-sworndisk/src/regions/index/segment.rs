use crate::{
    prelude::*,
    regions::{Checkpoint, Record},
    utils::*,
};

use crypto::{get_random_bytes, Aead};

#[derive(Debug)]
pub struct IndexSegment {
    pub hba: u64,
}

impl IndexSegment {
    pub fn new(hba: u64) -> Self {
        Self { hba }
    }

    pub fn write(
        &mut self,
        data: &[u8],
        len: usize,
        aead: &Pin<Box<Aead>>,
        client: &DmIoClient,
        _checkpoint: &mut Checkpoint,
        bdev: &BlockDevice,
    ) -> Result<Record> {
        let mut block = Vec::new();
        block.try_resize(BLOCK_SIZE as usize, 0u8)?;
        block[0..len].copy_from_slice(&data[0..len]);

        let key = get_random_bytes(SWORNDISK_KEY_LENGTH)?;
        let mut nonce = get_random_bytes(SWORNDISK_NONCE_LENGTH)?;
        let mac = unsafe {
            aead.as_ref()
                .encrypt_in_place(&key, &mut nonce, &mut block[..], BLOCK_SIZE as usize)?
        };

        let hba = self.hba;
        let mut region = DmIoRegion::new(&bdev, hba, BLOCK_SECTORS)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            WRITE as i32,
            WRITE as i32,
            block.as_mut_ptr() as *mut c_void,
            0,
            client,
        );
        io_req.submit(&mut region);

        self.hba += BLOCK_SECTORS;

        let record = Record {
            hba,
            key: vec_to_slice::<{ SWORNDISK_KEY_LENGTH }>(&key)?,
            nonce: vec_to_slice::<{ SWORNDISK_NONCE_LENGTH }>(&nonce)?,
            mac: vec_to_slice::<{ SWORNDISK_MAC_LENGTH }>(&mac)?,
        };

        Ok(record)
    }
}
