# SwornDisk Linux - Rust

SwornDisk is a log-structured secure block device. This implementation is in Rust for Linux.

## Components

- `async-work`: crate provides support for Linux concurrency managed work queue.
- `crypto`: crate provides support for Linux crypto API (currently only support AES-GCM)
- `device-mapper`: crate provides support for Linux Device Mapper ability 
- `dm-sworndisk`: SwornDisk device mapper target

## Usage

To build the kernel module, run:

```bash
make clean && make
```

To install the kernel module and mount a device mapper at `/dev/mapper/test-sworndisk`:

```bash
make modtest   # use `make restore` to restore
```

To run the unit test:

```bash
make unittest
```