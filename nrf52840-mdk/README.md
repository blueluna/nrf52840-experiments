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

## Examples

### Beacon

The `beacon` example will try to send a beacon request every thirty
seconds.

```
$ cargo run --example beacon_rtfm
```

### Associate

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
     | ------- acknowledge ------> |
     |                             |
```

Note that the coordinator must permit new associations to the PAN.

```
$ cargo run --example associate
```
