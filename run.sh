#!/bin/bash

qemu-system-x86_64 \
    -accel kvm \
    -no-reboot \
    -machine q35 \
    -cpu host \
    -smp 2 \
    -m 64M \
    -serial mon:stdio \
    -display none \
    -net none \
    -bios ./ovmf.fd \
    -drive format=raw,file=fat:rw:./.hdd/image/ \
    -drive format=raw,file=./.hdd/nvme.img,id=nvm,if=none \
    -device nvme,drive=nvm,serial=deadbeef \
    -D .debug/qemu_debug.log \
    -d int,guest_errors
