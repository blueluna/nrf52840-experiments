# Experiments with nRF52840-MDK

## Debug

I had to build openocd from source to get the DAPLink to function.

```
$ openocd -f openocd.conf 
```

Then run the program

```
$ cargo run --example beacon
```

cargo will use the run definition found in `.cargo/config` to launch `gdb` with
the `openocd.gdb` script file.
