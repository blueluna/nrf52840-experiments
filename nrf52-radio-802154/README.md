# IEEE 802.15.4 with nRF52840

Work in progress 802.15.4 radio fornRF52840-DK.

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

Use the `receive_rtfm` example to do recive only. The recieve only code is
fairly stable.

## Send beacon

The `beacon_rtfm` example will try to send a beacon request every thirty
seconds.

```
$ cargo run --example beacon_rtfm
```

## PAN Association

With this example there is an state machine which tries to send and receive
packets with the goal to associate with a PAN.

```text
   Device                      Coordinator
     |                             |
     | ----- beacon request -----> |
     |                             |
     | <--------- beacon --------- |
     |                             |
     | -- association request ---> |
     |                             |
     | <------ acknowledge ------- |
     |                             |
     | ------ data request ------> |
     |                             |
     | <-- association response -- |
     |                             |
```

Note that the coordinator must permit new associations to the PAN.

```
$ cargo run --example associate_rtfm
```

[JLinkGDBServer]:https://www.segger.com/products/debug-probes/j-link/tools/j-link-gdb-server/about-j-link-gdb-server/
