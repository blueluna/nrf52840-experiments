# nRF52840 AES for Psila 

Functions for using the ECB block and other AES implementations with the nRF52840.


This uses the nrf_cc310 library provided by Nordic in their SDK. Copy the
directory `external/nrf_cc310` from the SDK into this directory before
building.

The comparison for different AES-128 modes and implementations are as follows.

```
~~~ Run some tests ~~~
~~~ Encrypt HW AES ECB ~~~
SUCCESS
took 33 us
~~~ Encrypt HW AES CTR ~~~
SUCCESS
took 33 us
~~~ Decrypt HW AES CTR ~~~
SUCCESS
took 33 us
~~~ Encrypt HW AES CBC ~~~
SUCCESS
took 33 us
~~~ Encrypt Rust AES ECB ~~~
SUCCESS
took 517 us
~~~ Decrypt Rust AES ECB ~~~
SUCCESS
took 576 us
~~~ Encrypt ASM AES ECB ~~~
SUCCESS
took 83 us
~~~ Decrypt ASM AES ECB ~~~
SUCCESS
took 84 us
~~~ Encrypt ASM AES CTR ~~~
SUCCESS
took 87 us
~~~ Decrypt ASM AES CTR ~~~
SUCCESS
took 85 us
~~~ Encrypt ASM AES CBC ~~~
SUCCESS
took 85 us
~~~ Decrypt ASM AES CBC ~~~
SUCCESS
took 86 us
~~~ Encrypt CC AES ECB ~~~
SUCCESS
took 12 us
~~~ Decrypt CC AES ECB ~~~
SUCCESS
took 12 us
~~~ Encrypt CC AES CBC ~~~
SUCCESS
took 14 us
~~~ Decrypt CC AES CBC ~~~
SUCCESS
took 14 us
```

 * HW, Use the nRF52 ECB hardware block
 * Rust, Use https://lib.rs/crates/aes
 * ASM, Use https://github.com/Ko-/aes-armcortexm
 * CC, Use the nRF52840 CryptoCell library
