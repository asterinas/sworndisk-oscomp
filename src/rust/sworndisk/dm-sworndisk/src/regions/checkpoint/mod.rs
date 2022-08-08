pub mod bitc;
pub mod dst;
pub mod svt;

pub use bitc::*;
pub use dst::*;
pub use svt::*;

use crate::{
    prelude::*,
    utils::{Deserialize, Serialize},
};

/// SwornDisk Checkpoint Region
pub struct Checkpoint {
    /// Data Segment Validity Table
    pub data_svt: SVT,
    /// Index Segment Validity Table
    pub index_svt: SVT,
    /// Data Segment Table
    pub dst: Vec<DST>,
    /// Index of current active (memory buffered) data segment
    pub current_data_segment: usize,
    /// BIT Category
    pub bit_category: BITCategory,
}

impl Debug for Checkpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checkpoint").finish()
    }
}

impl Checkpoint {
    /// Create an empty checkpoint region. This is used in the init phase of SwornDisk.
    pub fn new(data_segs: u64, index_segs: u64) -> Result<Self> {
        let checkpoint = Checkpoint {
            bit_category: BITCategory::new()?,
            data_svt: SVT::new(data_segs)?,
            index_svt: SVT::new(index_segs)?,
            current_data_segment: 0,
            dst: {
                let mut dst_vec = Vec::try_with_capacity(data_segs as usize)?;
                for _ in 0..data_segs {
                    dst_vec.try_push(DST::new()?)?;
                }
                dst_vec
            },
        };

        Ok(checkpoint)
    }

    /// Write the checkpoint to disk.
    ///
    /// The layout of checkpoint region in the disk:
    ///
    /// - meta info of checkpoint region
    /// - current segment index
    /// - data SVT
    /// - index SVT
    /// - DST vector (last_modify + len + BitMap)
    /// - BITCategory
    pub fn write_to_disk(
        &self,
        bdev: &BlockDevice,
        client: &DmIoClient,
        checkpoint_hba: u64,
    ) -> Result {
        let current_data_segment =
            unsafe { mem::transmute::<usize, [u8; 8]>(self.current_data_segment) };
        let data_svt = self.data_svt.serialize()?;
        let index_svt = self.index_svt.serialize()?;
        let bit_category = self.bit_category.serialize()?;

        let mut dst_vec = Vec::new();
        for item in self.dst.iter() {
            dst_vec.try_extend_from_slice(&item.serialize()?)?;
        }

        let mut vec = Vec::new();
        vec.try_extend_from_slice(&current_data_segment)?;
        vec.try_extend_from_slice(&data_svt)?;
        vec.try_extend_from_slice(&index_svt)?;
        vec.try_extend_from_slice(&dst_vec)?;
        vec.try_extend_from_slice(&bit_category)?;

        let len = vec.len();
        let should_extend = !(len % SECTOR_SIZE as usize == 0) as usize;
        let sector_count = vec.len() / (SECTOR_SIZE as usize) + should_extend;
        vec.try_resize(sector_count * SECTOR_SIZE as usize, 0)?;

        let meta = CheckpointHelper {
            data_svt_len: data_svt.len(),
            index_svt_len: index_svt.len(),
            dst_size: self.dst.len(),
            dst_len: dst_vec.len(),
            bit_category_len: bit_category.len(),
            sector_number: sector_count,
        };
        let mut meta_vec = meta.serialize()?;

        pr_info!("checkpoint meta: {:?}", meta);
        pr_info!(
            "writting checkpoint of length {} ({} sectors) to meta_dev hba {}",
            vec.len(),
            vec.len() / SECTOR_SIZE as usize,
            checkpoint_hba
        );

        // write checkpoint meta info in a single sector
        let mut region = DmIoRegion::new(&bdev, checkpoint_hba, 1)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            WRITE as i32,
            WRITE as i32,
            meta_vec.as_mut_ptr() as *mut c_void,
            0,
            client,
        );
        io_req.submit(&mut region);

        // TODO: encryption
        let mut region = DmIoRegion::new(&bdev, checkpoint_hba + 1, sector_count as u64)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            WRITE as i32,
            WRITE as i32,
            vec.as_mut_ptr() as *mut c_void,
            0,
            client,
        );
        io_req.submit(&mut region);

        Ok(())
    }

    pub fn read_from_disk(
        bdev: &BlockDevice,
        client: &DmIoClient,
        checkpoint_hba: u64,
    ) -> Result<Self> {
        // read metainfo
        let mut meta_sector = Vec::new();
        meta_sector.try_resize(SECTOR_SIZE as usize, 0u8)?;
        let mut region = DmIoRegion::new(&bdev, checkpoint_hba, 1)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            READ as i32,
            READ as i32,
            meta_sector.as_mut_ptr() as *mut c_void,
            0,
            client,
        );
        io_req.submit(&mut region);
        let meta = CheckpointHelper::deserialize(&meta_sector[..CHECKPOINT_HELPER_SIZE])?;

        // read checkpoint body
        let mut body = Vec::new();
        body.try_resize(meta.sector_number * SECTOR_SIZE as usize, 0u8)?;
        let mut region = DmIoRegion::new(&bdev, checkpoint_hba + 1, meta.sector_number as u64)?;
        let mut io_req = DmIoRequest::with_kernel_memory(
            READ as i32,
            READ as i32,
            body.as_mut_ptr() as *mut c_void,
            0,
            client,
        );
        io_req.submit(&mut region);

        // read fields
        let mut index = 0;
        let current_data_segment =
            unsafe { mem::transmute::<[u8; 8], usize>(body[index..index + 8].try_into().unwrap()) };
        index += 8;

        let data_svt = SVT::deserialize(&body[index..index + meta.data_svt_len])?;
        index += meta.data_svt_len;

        let index_svt = SVT::deserialize(&body[index..index + meta.index_svt_len])?;
        index += meta.index_svt_len;

        // DST vector
        let mut dst = Vec::new();
        for _ in 0..meta.dst_size {
            let bvm_len = unsafe {
                mem::transmute::<[u8; 8], usize>(body[index + 8..index + 16].try_into().unwrap())
            };
            let total_len = 16 + bvm_len;
            let item = DST::deserialize(&body[index..index + total_len])?;
            dst.try_push(item)?;
            index += total_len;
        }

        // BIT category
        let bit_category = BITCategory::deserialize(&body[index..index + meta.bit_category_len])?;
        // index += meta.bit_category_len;

        Ok(Self {
            data_svt,
            index_svt,
            dst,
            current_data_segment,
            bit_category,
        })
    }

    pub fn debug(&self) {
        pr_info!("current segment: {}", self.current_data_segment);
        pr_info!("data_svt len: {}", self.data_svt.len());
        pr_info!("index_svt len: {}", self.index_svt.len());
        pr_info!("dst size: {}", self.dst.len());
        pr_info!("bit category len: {}", self.bit_category.len());
    }
}

#[derive(Debug)]
struct CheckpointHelper {
    data_svt_len: usize,
    index_svt_len: usize,
    dst_size: usize,
    dst_len: usize,
    bit_category_len: usize,
    sector_number: usize,
}

const CHECKPOINT_HELPER_SIZE: usize = mem::size_of::<CheckpointHelper>();

impl Serialize for CheckpointHelper {
    fn serialize(&self) -> Result<Vec<u8>> {
        let mut vec = Vec::new();
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.data_svt_len) })?;
        vec.try_extend_from_slice(&unsafe {
            mem::transmute::<usize, [u8; 8]>(self.index_svt_len)
        })?;
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.dst_size) })?;
        vec.try_extend_from_slice(&unsafe { mem::transmute::<usize, [u8; 8]>(self.dst_len) })?;
        vec.try_extend_from_slice(&unsafe {
            mem::transmute::<usize, [u8; 8]>(self.bit_category_len)
        })?;
        vec.try_extend_from_slice(&unsafe {
            mem::transmute::<usize, [u8; 8]>(self.sector_number)
        })?;
        // todo: checksum
        vec.try_resize(CHECKPOINT_HELPER_SIZE, 0u8)?;
        Ok(vec)
    }
}

impl Deserialize for CheckpointHelper {
    fn deserialize(buf: &[u8]) -> Result<Self> {
        if buf.len() != CHECKPOINT_HELPER_SIZE {
            return Err(EINVAL);
        }

        let data_svt_len =
            unsafe { mem::transmute::<[u8; 8], usize>(buf[0..8].try_into().unwrap()) };
        let index_svt_len =
            unsafe { mem::transmute::<[u8; 8], usize>(buf[8..16].try_into().unwrap()) };
        let dst_size = unsafe { mem::transmute::<[u8; 8], usize>(buf[16..24].try_into().unwrap()) };
        let dst_len = unsafe { mem::transmute::<[u8; 8], usize>(buf[24..32].try_into().unwrap()) };
        let bit_category_len =
            unsafe { mem::transmute::<[u8; 8], usize>(buf[32..40].try_into().unwrap()) };
        let sector_number =
            unsafe { mem::transmute::<[u8; 8], usize>(buf[40..48].try_into().unwrap()) };

        Ok(Self {
            data_svt_len,
            index_svt_len,
            dst_size,
            dst_len,
            bit_category_len,
            sector_number,
        })
    }
}
