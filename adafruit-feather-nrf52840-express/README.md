# IEEE 802.15.4 with nRF52840

Work in progress 802.15.4 radio for Adafruit feather nRF52840 express.

## Running

These examples use `probe-run` to flash and run them. For example,

```
DEFMT_LOG=info cargo run --example feather-express-psila
```

## Examples

### Listener

Listen for 802.15.4 messages and sending them to the host using serial.

### Psila

A Zigbee colour light
