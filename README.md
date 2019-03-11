# Experiments with nRF52840-DK

This is some experiments with using the nrf52840 radio in 802.14.5 mode.

There is a target (nRF52840-DK) part, a host part and a serialiser/deserialiser
part. The nRF52840 communicates with the host over the USB-serial transport.

## Parts

### Serialiser / deserialiser

`esercom` is a small serialise / deserialise library for sending data over the
serial line.

### Target

The target part is found in the `nrf52-radio-802145` directory.

### Host

The host part is found in the `host` directory.

## Usage

 1. Start the host application listening to the nrf52840-DK USB-to-serial device
 2. Start the target application on the nRF52840-DK

## License

Licensed under the MIT license. See LICENSE.
