# Firmware

Prerequisites:
```
apt install openocd gdb-multiarch
```

Connect STlink v2, only ground, swdio & swclk, leave 3v3 unconnected.

In one terminal, from this directory run;
```
openocd
```

In the other terminal one can program with;
```
cargo r --release
```

Testing:
```
cargo t --release --target x86_64-unknown-linux-gnu
```

## USB pullup
The board I used had a 10k pullup resistor on R10, which made the USB bus non-functional, added a 2.2k 0603 in parallel
to drop it to 1.8k resistor in total, which works.
