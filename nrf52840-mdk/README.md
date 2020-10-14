# Experiments with nRF52840-MDK

Work in progress 802.15.4 radio for Makerdiary nRF52840-MDK.

## Running

These examples use cargo embed to run them. For example,

```
cargo embed --target thumbv7em-none-eabihf --example nrf52840-mdk-listener
```

## Examples

### Listener

Listen for 802.15.4 messages and sending them to the host using serial.
