# IEEE 802.15.4 with nRF52840

Work in progress 802.15.4 radio for nRF52840-DK.

## Running

These examples use `probe-run` to flash an run them. For example,

```
DEFMT_LOG=info cargo run --example nrf52840-dk-psila
```

## Examples

### Blinky

Simple led and button example

### Energy Detect

Exploring energy detect feature of the nRF52 radio.

### Listener

Listen for 802.15.4 messages and sending them to the host using serial.

### Psila

A Zigbee on/off light
