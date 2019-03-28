# Work in progress IEEE 802.15.4 for nRF52840

This is some experiments with using the nRF52840 radio in 802.14.5 mode. The
examples in this repository assumes that one of the nRF52840-DK,
nRF52840-dongle, nRF52840-MDK boards is used.
The host program has only been tested with Fedora 29 Linux.

The code is split into following parts.

## Parts

### Serialiser / deserialiser

`esercom` is a small serialise / deserialise library for sending data over the
serial line.

### nRF52840 radio

`nrf52-radio-802154` is a library for using the nRF52480 radio peripheral in
IEEE 802.15.4 mode.

### Target

The target examples are found in the `nrf52840-dk`, `nrf52840-dongle` and
`nrf52840-mdk` directories.

There is no serial link implemented for nRF52840-dongle.

### Host

The host tool is found in the `host` directory.

## Usage

 1. Start the host application listening to the nrf52840 USB-to-serial device
 2. Start the target application on the nRF52840

## License

Licensed under the MIT license. See LICENSE.
