# Experiments with nRF52840

Experiments with nRF52840-DK

## Debug

[JLinkGDBServer] from Segger is used to debug, see the `jlinkgdb` shell script
on how JLinkGDBServer is invoked.

Start the GDB server with `jlinkgdb`.

```
$ ./jlinkgdb
```

Then run the program

```
$ cargo run --example receive_rtfm
```

cargo will use the run definition found in `.cargo/config` to launch `gdb` with
the `jlink.gdb` script file.

## Receive only

Use the `receive_rtfm` example to do recive only. The recieve only code is fairly stable.

## Send beacon

The is also a `beacon_rtfm` example, this will try to send a beacon request every sixty seconds.
This example seems to fail shortly after sending the beacon request.

[JLinkGDBServer]:https://www.segger.com/products/debug-probes/j-link/tools/j-link-gdb-server/about-j-link-gdb-server/
