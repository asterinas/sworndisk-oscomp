#!/usr/bin/env bash

# --------------------
# Configurations
# --------------------
# include demo-magic.sh
. /home/bellaris/demoscript/demo-magic/demo-magic.sh

# set typing speed
TYPE_SPEED=20

# set terminal prompt style
DEMO_PROMPT="${GREEN}➜ ${CYAN}\W "

clear

# -------------------
# Begin Demo Script
# -------------------

pe "ls"
echo ''

p "# 编译 SwornDisk Linux Rust"
pe "make clean"
pe "make"
wait
clear

p "# 创建磁盘文件 (30G 数据 + 8G 元信息)"
pe "dd if=/dev/null of=~/tmp/disk.img seek=58593750"
pe "dd if=/dev/null of=~/tmp/meta.img seek=8388608"
echo ''

p "# 挂载为回环设备"
pe "sudo losetup /dev/loop0 ~/tmp/disk.img"
pe "sudo losetup /dev/loop1 ~/tmp/meta.img"
echo ''

p "# 加载 SwornDisk Linux Rust 内核模块"
pe "modinfo dm-sworndisk.ko"
pe "sudo insmod dm-sworndisk.ko"
echo ''

p "# 创建 SwornDisk Device Mappper 设备"
pe "echo -e \"0 58593750 sworndisk /dev/loop0 /dev/loop1 0 true\" | sudo dmsetup create sworndisk"
pe "ls -l /dev/mapper"
echo ''

p "# 运行 fio 性能测试"
pe "sudo fio -ioengine=sync -size=4G -iodepth=32 -rw=randwrite -filename=/dev/mapper/sworndisk -name=randwrite -bs=4K -direct=1 -numjobs=1 -fsync_on_close=1"
echo ''
