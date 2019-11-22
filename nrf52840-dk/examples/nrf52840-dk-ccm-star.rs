#![no_main]
#![no_std]

use core::sync::atomic::{self, Ordering};

#[allow(unused_imports)]
use panic_itm;

use cortex_m::{iprint, iprintln, peripheral::ITM};

use rtfm::app;

use nrf52840_hal::{clocks, prelude::*};

use nrf52_cryptocell::{self, CryptoCellBackend};
use psila_crypto::{self, CryptoBackend};
use psila_data::{
    application_service::ApplicationServiceHeader,
    pack::{Pack, PackFixed},
    security::{SecurityHeader, SecurityLevel},
};

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        crypto: CryptoCellBackend,
        itm: ITM,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        // Configure to use external clocks, and start them
        let _clocks = cx
            .device
            .CLOCK
            .constrain()
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let cryptocell = CryptoCellBackend::new(cx.device.CRYPTOCELL);

        init::LateResources {
            crypto: cryptocell,
            itm: cx.core.ITM,
        }
    }

    #[idle(resources = [itm, crypto])]
    fn idle(cx: idle::Context) -> ! {
        let itm_port = &mut cx.resources.itm.stim[0];
        let crypto = cx.resources.crypto;

        iprintln!(itm_port, "~~~ Run some tests ~~~");

        {
            let key = [
                0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD,
                0xCE, 0xCF,
            ];
            let nonce = [
                0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0x03, 0x02, 0x01, 0x00, 0x06,
            ];
            let message = [
                0x1A, 0x55, 0xA3, 0x6A, 0xBB, 0x6C, 0x61, 0x0D, 0x06, 0x6B, 0x33, 0x75, 0x64, 0x9C,
                0xEF, 0x10, 0xD4, 0x66, 0x4E, 0xCA, 0xD8, 0x54, 0xA8, 0x0A, 0x89, 0x5C, 0xC1, 0xD8,
                0xFF, 0x94, 0x69,
            ];
            let additional_data = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
            // M, length of the authentication field in octets 0, 4, 6, 8, 10, 12, 14, 16
            const MIC_LENGTH: usize = 8;
            let mut output = [0u8; 128];

            let (payload, mic) = message.split_at(message.len() - MIC_LENGTH);

            match crypto.ccmstar_decrypt(
                &key,
                &nonce,
                &payload,
                &mic,
                &additional_data,
                &mut output,
            ) {
                Ok(_) => {
                    if output[..message.len() - MIC_LENGTH]
                        == [
                            0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13,
                            0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E,
                        ]
                    {
                        iprintln!(itm_port, "CCM Test 1 succeded");
                    } else {
                        iprintln!(itm_port, "CCM Test 1 failed, Mismatching output");
                    }
                }
                Err(e) => {
                    iprintln!(itm_port, "CCM Test 1 failed");
                    if let nrf52_cryptocell::Error::Other(errno) = e {
                        iprintln!(itm_port, "CC Error {:08x}", errno);
                    }
                }
            }
        }

        {
            /*
            let input = [
                0x21, 0xf2, 0x30, 0x05, 0x00, 0x00, 0x00, 0xb5, 0xb4, 0x03, 0xff, 0xff, 0x2e, 0x21,
                0x00, 0x63, 0xe2, 0x62, 0xd6, 0xb3, 0x67, 0x4d, 0x0e, 0x34, 0x9f, 0xaa, 0x04, 0x81,
                0xf9, 0x1d, 0xf6, 0xa4, 0x72, 0x7f, 0x36, 0xde, 0x4d, 0xf5, 0xeb, 0xd8, 0xea, 0xc5,
                0x4e, 0x78, 0x1c, 0xd9, 0x36, 0x07, 0xb4, 0x62, 0xc9, 0xf8, 0xb7, 0x77
            ];
             */

            let input = [
                0x21, 0x45, 0x30, 0x02, 0x00, 0x00, 0x00, 0x38, 0x2e, 0x03, 0xff, 0xff, 0x2e, 0x21,
                0x00, 0xae, 0x5e, 0x9f, 0x46, 0xa6, 0x40, 0xcd, 0xe7, 0x90, 0x2f, 0xd6, 0x0e, 0x43,
                0x23, 0x17, 0x48, 0x4b, 0x4c, 0x5a, 0x9b, 0x4c, 0xde, 0x1c, 0xe7, 0x07, 0x07, 0xb6,
                0xfb, 0x1a, 0x0b, 0xe9, 0x99, 0x7e, 0x0a, 0xf8, 0x0f, 0xdf, 0x5d, 0xcf,
            ];

            let (_aps, aps_used) = ApplicationServiceHeader::unpack(&input).unwrap();
            let (mut sec, sec_used) = SecurityHeader::unpack(&input[aps_used..]).unwrap();

            sec.control.set_level(SecurityLevel::EncryptedIntegrity32);

            let mut buffer = [0u8; 256];

            let key = [
                0x4bu8, 0xab, 0x0f, 0x17, 0x3e, 0x14, 0x34, 0xa2, 0xd5, 0x72, 0xe1, 0xc1, 0xef,
                0x47, 0x87, 0x82,
            ];

            buffer[..input.len()].copy_from_slice(&input);

            sec.control.pack(&mut buffer[aps_used..=aps_used]).unwrap();

            let mut nonce = [0u8; 13];
            let _ = sec.get_nonce(&mut nonce);

            let mic_bytes = sec.control.level.mic_bytes();

            let aad_size = aps_used + sec_used;
            let aad = &buffer[..aad_size];

            let payload_end = input.len() - mic_bytes;
            let payload = &buffer[aad_size..payload_end];

            let mic = &buffer[payload_end..input.len()];

            let mut output = [0u8; 128];

            match crypto.ccmstar_decrypt(&key, &nonce, &payload, &mic, &aad, &mut output) {
                Ok(size) => {
                    if size == 35 {
                        let correct_output = [
                            0x05, 0x01, 0x00, 0x2c, 0x6c, 0x08, 0xd0, 0xf4, 0xf4, 0x2c, 0xd8, 0x40,
                            0xd8, 0x48, 0x00, 0x40, 0x64, 0x08, 0x00, 0x85, 0xae, 0x21, 0xfe, 0xff,
                            0x6f, 0x0d, 0x00, 0x38, 0x2e, 0x03, 0xff, 0xff, 0x2e, 0x21, 0x00,
                        ];
                        if output[..16] == correct_output[..16]
                            && output[16..size] == correct_output[16..]
                        {
                            iprintln!(itm_port, "CCM Test 2 succeded");
                        } else {
                            iprintln!(itm_port, "CCM Test 2, Incorrect response");
                        }
                    } else {
                        iprintln!(itm_port, "CCM Test 2, Incorrect length {}", size);
                    }
                }
                Err(e) => {
                    iprintln!(itm_port, "Failed to decrypt payload");
                    if let nrf52_cryptocell::Error::Other(errno) = e {
                        iprintln!(itm_port, "CC Error {:08x}", errno);
                    }
                }
            }
        }

        {
            let messagge = [
                0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
                0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E,
            ];
            let key = [
                0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD,
                0xCE, 0xCF,
            ];
            let nonce = [
                0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0x03, 0x02, 0x01, 0x00, 0x06,
            ];
            let aad = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];

            let mut output = [0u8; 128];
            let mut mic = [0u8; 8];

            match crypto.ccmstar_encrypt(&key, &nonce, &messagge, &mut mic, &aad, &mut output) {
                Ok(size) => {
                    if size == 23 {
                        let correct_output = [
                            0x1A, 0x55, 0xA3, 0x6A, 0xBB, 0x6C, 0x61, 0x0D, 0x06, 0x6B, 0x33, 0x75,
                            0x64, 0x9C, 0xEF, 0x10, 0xD4, 0x66, 0x4E, 0xCA, 0xD8, 0x54, 0xA8,
                        ];
                        let correct_mic = [0x0A, 0x89, 0x5C, 0xC1, 0xD8, 0xFF, 0x94, 0x69];
                        if output[..16] == correct_output[..16]
                            && output[16..size] == correct_output[16..]
                            && mic == correct_mic
                        {
                            iprintln!(itm_port, "CCM Test 3 succeded");
                        } else {
                            for (a, b) in mic.iter().zip(correct_mic.iter()) {
                                iprint!(itm_port, "{:02x} {:02x} ", a, b);
                            }
                            iprintln!(itm_port, "CCM Test 3, Incorrect response");
                        }
                    } else {
                        iprintln!(itm_port, "CCM Test 3, Incorrect length {}", size);
                    }
                }
                Err(e) => {
                    iprintln!(itm_port, "Failed to decrypt payload");
                    if let nrf52_cryptocell::Error::Other(errno) = e {
                        iprintln!(itm_port, "CC Error {:08x}", errno);
                    }
                }
            }
        }

        loop {
            atomic::compiler_fence(Ordering::SeqCst)
        }
    }
};
