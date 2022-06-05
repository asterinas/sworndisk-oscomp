echo 'Setting up loop device /dev/loop0'
sudo losetup /dev/loop0 /home/bellaris/tmp/disk.img

echo 'Setting up loop device /dev/loop1'
sudo losetup /dev/loop1 /home/bellaris/tmp/metadisk.img

echo 'Setting up kernel module...'
sudo insmod dm-sworndisk.ko

echo 'Setting up dm_sworndisk device mapper...'
echo -e "0 58593750 sworndisk /dev/loop0 /dev/loop1 0 $FORMAT" | sudo dmsetup create test-sworndisk