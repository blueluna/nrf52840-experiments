# Experiments with nRF52840

Experiments with nRF52840-DK

## Current limitations

 * The turnaround for RX to RX and TX to RX seems to take to long time,
   sometimes the receiver fail to receive packets.
 * The examples doesn't seem to work when built as release.

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

Unfortunately this doesn't seem to work since the acknowledge after sending
the association request isn't picked up by the radio (slow TX to RX
turnaround?)

[JLinkGDBServer]:https://www.segger.com/products/debug-probes/j-link/tools/j-link-gdb-server/about-j-link-gdb-server/
