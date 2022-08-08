// SPDX-License-Identifier: GPL-2.0

//! # SwornDisk Linux Rust
//!
//! Rust implementation of dm-sworndisk.

#![feature(const_fn_trait_bound)]
#![allow(dead_code)]

extern crate alloc;
extern crate kernel;

mod constant;
mod context;
mod handler;
mod prelude;
mod regions;
mod types;
mod unittest;
mod utils;
mod workers;

use prelude::*;

use handler::DmSwornDiskHandler;

module! {
    type: DmSwornDisk,
    name: b"dm_sworndisk",
    author: b"Occlum Team",
    description: b"Rust implementation of SwornDisk based on Linux device mapper.",
    license: b"GPL v2",
    params: {
        run_unittest: bool {
            default: false,
            permissions: 0,
            description: b"Run dm-sworndisk kernel module unit test",
        },
    },
}

struct DmSwornDisk {
    _target: Pin<Box<TargetType>>,
}

impl KernelModule for DmSwornDisk {
    fn init(_name: &'static CStr, _module: &'static ThisModule) -> Result<Self> {
        pr_info!("Loading SwornDisk kernel module");

        let name = c_str!("sworndisk");
        let version = [1, 0, 0];
        let features = 0;

        // Unit test in kernel module
        {
            let should_run_unittest = run_unittest.read();
            if *should_run_unittest {
                unittest::run_all_test();
            }
        };

        let mut sworndisk_target = TargetType::new_pinned(name, version, features, _module)?;
        sworndisk_target.as_mut().register::<DmSwornDiskHandler>();

        Ok(DmSwornDisk {
            _target: sworndisk_target,
        })
    }
}

impl Drop for DmSwornDisk {
    fn drop(&mut self) {}
}
