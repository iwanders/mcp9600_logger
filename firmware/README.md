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
