# Running

```bash
make run

# after the qemu shell opens:
fs0:
boot.efi
```

# Testing

```bash
make debug

# after the qemu shell opens:
fs0:
boot.efi

# in another terminal window:
telnet localhost 1235
info registers
```
You should see that register `x1` has the value of `0xbeef`, set in
`kernel/src/main.rs`.
