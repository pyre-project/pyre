"C:\Program Files\qemu\qemu-system-x86_64.exe"^
    -no-reboot^
    -machine q35^
    -cpu max^
    -smp 2^
    -m 2G^
    -serial stdio^
    -net none^
    -bios ./resources/ovmf.fd^
    -drive format=raw,file=fat:rw:./.hdd/image/^
    -drive format=raw,file=./.hdd/nvme.img,id=nvm,if=none^
    -device nvme,drive=nvm,serial=deadbeef

