PWD := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
ROOT := $(PWD)/build/root
UEFI := $(PWD)/uefi
KERNEL := $(PWD)/kernel
BUILD_STD := -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem
QEMU_FLAGS := -machine virt -cpu max \
	-drive if=pflash,format=raw,file=$(PWD)/qemu_efi.fd \
	-drive format=raw,file=fat:rw:$(ROOT) \
	-net none \
	-nographic

.PHONY: clean default build boot.efi kernel.elf run

default: build

boot.efi:
	cargo build --manifest-path $(UEFI)/Cargo.toml --release --target aarch64-unknown-uefi $(BUILD_STD)

kernel.elf:
	cargo build --manifest-path $(KERNEL)/Cargo.toml --release --target aarch64-unknown-none $(BUILD_STD)

build: boot.efi kernel.elf
	mkdir -p $(PWD)/build
	mkdir -p $(PWD)/build/root/efi/boot
	cp $(PWD)/target/aarch64-unknown-uefi/release/uefi.efi $(ROOT)/boot.efi
	cp $(PWD)/target/aarch64-unknown-none/release/kernel $(ROOT)/kernel.elf

clippy:
	cargo clippy --manifest-path $(UEFI)/Cargo.toml --release --target aarch64-unknown-uefi $(BUILD_STD)
	cargo clippy --manifest-path $(KERNEL)/Cargo.toml --release --target aarch64-unknown-none $(BUILD_STD)

run: build
	qemu-system-aarch64 $(QEMU_FLAGS)
		
debug: build
	qemu-system-aarch64 $(QEMU_FLAGS) \
		-monitor telnet:localhost:1235,server,nowait \
		-gdb tcp::1236

clean:
	rm -rf $(PWD)/build
	cargo clean --manifest-path $(KERNEL)/Cargo.toml
	cargo clean --manifest-path $(UEFI)/Cargo.toml
