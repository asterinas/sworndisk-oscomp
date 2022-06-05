# SPDX-License-Identifier: GPL-2.0
LD = ld.lld
MODULE_NAME = dm-sworndisk
CRATE_NAME = dm_sworndisk
PWD = $(shell pwd)
KDIR = /lib/modules/`uname -r`/build
KCONFIG = CONFIG_DM_BUFIO=m CONFIG_DM_PERSISTENT_DATA=m CONFIG_LIBCRC32C=m CONFIG_SWORNDISK=m
OUTPUT_DIR = $(PWD)/target/target/release/deps

# .SILENT:

ALL: dm-sworndisk.ko

dm-sworndisk.ko:
	$(MAKE) cargo-build
	sh scripts/generate_cmd.sh
	LLVM=1 $(MAKE) $(KCONFIG) -C $(KDIR) M=$$PWD

cargo-build: dm-sworndisk/src/lib.rs
	RUST_MODFILE=$(PWD) cargo build --release
	$(LD) -r $$(ls -d $(OUTPUT_DIR)/* | grep '.*[\.o]$$') -o ./$(MODULE_NAME).o
	cp $(OUTPUT_DIR)/$$(ls $(OUTPUT_DIR) | grep '$(CRATE_NAME).*[\.d]$$') ./.$(MODULE_NAME).d

clean:
	cargo clean
	rm -rf .*.cmd .*.d
	rm -rf *.order *.symvers *.ko *.o *.mod *.c bindings_generated.rs *.d *.tmp

modtest: $(MODULE_NAME).ko scripts/insmod.sh
	FORMAT=true scripts/insmod.sh

forceformat: $(MODULE_NAME).ko scripts/insmod.sh
	FORMAT=force scripts/insmod.sh

unittest: $(MODULE_NAME).ko
	sudo insmod ./$(MODULE_NAME).ko run_unittest=true

restore: $(MODULE_NAME).ko scripts/restore.sh
	scripts/restore.sh

fio: $(MODULE_NAME).ko scripts/fio.conf
	sudo fio scripts/fio.conf
