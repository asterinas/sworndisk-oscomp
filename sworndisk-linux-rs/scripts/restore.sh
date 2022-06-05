#!/bin/bash
echo 'Remove Device Mapper test-linear...'
sudo dmsetup remove test-sworndisk

echo 'Remove kernel module...'
sudo rmmod dm-sworndisk.ko

echo 'Remove loop device...'
sudo losetup -d /dev/loop0
sudo losetup -d /dev/loop1
