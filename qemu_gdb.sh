qemu-system-gnuarmeclipse --verbose --verbose --board STM32F4-Discovery \
    --mcu STM32F407VG --gdb tcp::1234 -d unimp,guest_errors \
    --semihosting-config enable=on,target=native \
    --semihosting-cmdline main
