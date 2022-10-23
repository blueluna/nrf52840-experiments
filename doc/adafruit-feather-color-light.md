# The Adafruit Feather nRF52840 Express Color Light Demo

## Preperations

### Required hardware

Following is needed for this demo.

 * Adafruit Feather nRF52840 Express with a JTAG header
 * Segger J-Link to flash the board
 * Conbee II or Conbee

## Rust

The rust toolchain is required. See <https://rustup.rs/>.

### probe-run

`probe-run` is used to flash and run the examples. It can be installed through `cargo`,
It might need additional system libraries.


```
cargo install -f probe-run
```

To configure permissions to use the debug probe, See the [probe-rs setting started guide](https://probe.rs/docs/getting-started/probe-setup/).

### deCONZ

Download and install deCONZ.

<https://www.phoscon.de/en/conbee2/install>

### nRF5 SDK

The Cryptocell crate requires parts of the nRF5 SDK.

[Download the nRF5 SDK](https://www.nordicsemi.com/Products/Development-software/nRF5-SDK/Download). And follow additional instructions later.

## Setup deCONZ

Connect the Conbee. Start deCONZ.

Connect using the Conbee.

### Configure channel mask

The example will use channel 15. To change the channel in deCONZ, do following

 * Leave the network.
 * Change network settings, change channel mask to channel 15.
 * Save and Done.
 * Join the network again.

To verify that channel 15 is in use, open the network settings again.

### Phoscon App

Click the `Phoscon App` button in the upper right corner. Select the Conbee and register an account.

## Building the example

Get the code,

```
git clone https://github.com/blueluna/nrf52840-experiments.git
```

Unzip the nRF5 SDK archive. Then copy the `external/nrf_cc310` directory into
`nrf52-cryptocell`.

```
cp -r ~/Downloads/nRF5_SDK_17.1.0_ddde560/external/nrf_cc310 nrf52-cryptocell/
```

Build the examples.

```
cargo build --examples --release
```

## Running the demo

Go into the `adafruit-feather-nrf52840-express` directory. Flash and run the example.

```
DEFMT_LOG=info cargo run --release --example feather-express-psila
```

The output should look as follows.

```
erik@computer:~/rust/nrf52840-experiments/adafruit-feather-nrf52840-express$ DEFMT_LOG=info cargo run --release --example feather-express-psila
    Finished release [optimized] target(s) in 0.03s
     Running `probe-run --probe '1366:0101' --chip nRF52840_xxAA ../nrf52840-experiments/target/thumbv7em-none-eabihf/release/examples/feather-express-psila`
(HOST) WARN  insufficient DWARF info; compile your program with `debug = 2` to enable location info
(HOST) INFO  flashing program (21 pages / 84.00 KiB)
(HOST) INFO  success!
────────────────────────────────────────────────────────────────────────────────
0 INFO  MAC: Send beacon request
1 INFO  MAC: Association failed, retry
```

Go to the Phoscon App.

 * Create a group.
 * Click Edit -> Manage Light in the group.
 * Click Add new light.

![Add new light](images/new_light.jpg)

Go to the program execution. The device should find the controller and associate with it.

```
0 INFO  MAC: Send beacon request
1 INFO  mac: Beacon 6754:0
2 INFO  MAC: Association failed, retry
3 INFO  MAC: Send beacon request
4 INFO  mac: Beacon 6754:0 permit join
5 INFO  MAC: Send association request
6 INFO  MAC: Acknowledge 3
7 INFO  MAC: Send data request
8 INFO  MAC: Acknowledge 4
9 INFO  MAC: Association Response, Success, 6754:45864
10 INFO  Key-transport key
11 INFO  > APS Set network key
12 INFO  > DP Device announce
13 INFO  APS acknowledge request, extended
14 INFO  < Queued acknowledge 43
...
```

Go to the Phoscon App.

The search should have found the device, add the device to the group.

![Found new light](images/found.jpg)

Go back to the main page. The device should appear under the Light tab. Control on/off, brightness and color here.

![Control new light](images/control.jpg)

Note: The device does not store the exchanged keys, this means that the device needs to re-associate when restarted.