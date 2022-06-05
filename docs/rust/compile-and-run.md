# SwornDisk Linux Rust 编译 & 测试指南

# 编译使用

## Step 1. 获取 SwornDisk Linux Rust 源代码

```bash
$ git clone git@gitlab.eduxiji.net:lnhoo/project788067-120160.git sworndisk
```

## Step 2. 获取 rust-for-linux 源代码

SwornDisk 对 Rust 的基础支持基于 [rust-for-linux](https://github.com/rust-for-linux/linux) 项目。需要先基于此项目编译并替换当前 Linux 系统使用的内核。

首先拉取 rust-for-linux 的代码。由于 rust-for-linux 项目处于持续的迭代中，并且我们在此基础上做出了一定的修改，因此需要将 rust-for-linux 当前的仓库回溯到某个时间点 (486c2cde073e5d91d78f85d0adf9a911dd6775fa).

```bash
# 克隆 rust-for-linux 源码
$ git clone git@github.com:Rust-for-Linux/linux.git
$ cd linux

# 将代码回溯到某个确定的时间点
$ git checkout 486c2cde073e5d91d78f85d0adf9a911dd6775fa
```

## Step 3. 应用 SwornDisk 对 rust-for-linux 的修改

```bash
# 应用对 rust-for-linux 修改的 git patch
$ git apply ../sworndisk/rust/rust-for-linux-modification.patch

# 复制 SwornDisk Linux Rust 源码到 Linux 目录中
$ mkdir modules
$ cp ../sworndisk/rust/sworndisk modules/sworndisk
```

## Step 4. 编译具有 Rust 支持的 Linux 内核

参照 [rust-for-linux Quick Start](https://github.com/Rust-for-Linux/linux/blob/rust/Documentation/rust/quick-start.rst) 文档，编译具有 Rust 支持的 Linux 内核。

> **特别注意：SwornDisk 使用的 Rust 版本为 `rustc 1.60.0-nightly (9ad5d82f8 2022-01-18)`，在根据上述指南安装 Rust 工具链时，请务必切换到此版本。**

编译 Linux 内核时，请确保打开以下选项：

```
CONFIG_RUST_IS_AVAILABLE=y 
CONFIG_RUST=y 
CONFIG_DM_PERSISTENT_DATA=y 
CONFIG_DM_BUFIO=y 
CONFIG_LIBCRC32C=y 
CONFIG_BLK_DEV_LOOP=y 
CONFIG_BLK_DEV_DM=y 
CONFIG_BLK_DEV_LOOP_MIN_COUNT=8
```


## Step 5. 编译 SwornDisk Linux Rust

```sh
$ cd modules/sworndisk
$ make clean && make
```

## Step 6. 加载 SwornDisk 内核模块

```sh
$ sudo insmod dm-sworndisk.ko
```

## Step 7. 创建 SwornDisk 虚拟映射块设备

- `<size>`: 磁盘扇区数量，扇区大小为 512B
- `<data_dev>`: 数据磁盘对应设备文件
- `<meta_dev>`: 元数据磁盘对应设备文件
- `<format>`: 是否格式化创建磁盘：(force: 强制格式化创建新磁盘, true: 损坏时格式化, false: 不格式化)
- `<name>`: 磁盘名称

```bash
$ echo -e '0 <size> sworndisk <data_dev> <meta_dev> 0 force' | sudo dmsetup create <name>
```

示例：

```bash
# 创建一个 30GB 的 SwornDisk 虚拟块设备并格式化，位置是 /dev/mapper/test-sworndisk
$ echo -e '0 58593750 sworndisk /dev/loop0 /dev/loop1 0 force' | sudo dmsetup create test-sworndisk
```

# 性能测试

使用 fio 性能测试参考 config:

```conf
# fio.conf

[global]
ioengine=sync
thread=1
norandommap=1
randrepeat=0
runtime=60
ramp_time=6
size=4G
direct=1
filename=/dev/mapper/test-sworndisk

[write4k-rand]
stonewall
group_reporting
bs=4k
rw=randwrite
numjobs=1
iodepth=32

[write64k-seq]
stonewall
group_reporting
bs=64k
rw=write
numjobs=1
iodepth=32

[read4k-rand]
stonewall
group_reporting
bs=4k
rw=randread
numjobs=1
iodepth=32

[read64k-seq]
stonewall
group_reporting
bs=64k
rw=read
numjobs=1
iodepth=32
```

```bash
$ fio fio.conf
```