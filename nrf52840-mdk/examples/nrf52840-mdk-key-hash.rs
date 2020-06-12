#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::{hprint, hprintln};
use rtic::app;

use nrf52840_hal::clocks;

use nrf52_cryptocell::{self, CryptoCellBackend};
pub use psila_crypto::{self, CryptoBackend, Error};

/// Key length
pub const KEY_SIZE: usize = 16;

/// Cipher block length
pub const BLOCK_SIZE: usize = 16;

pub struct SecurityService<Backend> {
    pub backend: Backend,
}

impl<Backend> SecurityService<Backend>
where
    Backend: CryptoBackend,
{
    pub fn new(backend: Backend) -> Self {
        Self { backend }
    }

    /// Process a block for the Key-hash hash function
    fn hash_key_process_block(
        &mut self,
        input: &[u8],
        mut output: &mut [u8],
        finish: bool,
    ) -> Result<(), nrf52_cryptocell::Error> {
        self.backend.aes128_ecb_encrypt_set_key(output)?;
        if finish {
            self.backend
                .aes128_ecb_encrypt_finish(&input, &mut output)?;
        } else {
            self.backend
                .aes128_ecb_encrypt_process_block(&input, &mut output)?;
        }
        // XOR the input into the hash block
        for n in 0..BLOCK_SIZE {
            output[n] ^= input[n];
        }
        Ok(())
    }

    /// Key-hash hash function
    fn hash_key_hash(&mut self, input: &[u8], output: &mut [u8]) -> Result<(), Error> {
        assert!(input.len() < 4096);

        // Clear the first block of output
        for b in output[..BLOCK_SIZE].iter_mut() {
            *b = 0;
        }

        let mut blocks = input.chunks_exact(BLOCK_SIZE);

        // Process input data in cipher block sized chunks
        loop {
            match blocks.next() {
                Some(input_block) => {
                    self.hash_key_process_block(&input_block, &mut output[..BLOCK_SIZE], false)?;
                }
                None => {
                    let mut block = [0u8; BLOCK_SIZE];
                    let remainder = blocks.remainder();
                    assert!(remainder.len() < BLOCK_SIZE - 3);
                    block[..remainder.len()].copy_from_slice(remainder);
                    block[remainder.len()] = 0x80;
                    let input_len = input.len() as u16 * 8;
                    // Append the data length to the end
                    block[BLOCK_SIZE - 2] = (input_len >> 8) as u8;
                    block[BLOCK_SIZE - 1] = (input_len & 0xff) as u8;
                    self.hash_key_process_block(&block, &mut output[..BLOCK_SIZE], true)?;
                    break;
                }
            }
        }
        Ok(())
    }

    /// FIPS Pub 198 HMAC?
    pub fn hash_key(
        &mut self,
        key: &[u8; KEY_SIZE],
        input: u8,
        result: &mut [u8],
    ) -> Result<(), nrf52_cryptocell::Error> {
        const HASH_INNER_PAD: u8 = 0x36;
        const HASH_OUTER_PAD: u8 = 0x5c;
        let mut hash_in = [0; BLOCK_SIZE * 2];
        let mut hash_out = [0; BLOCK_SIZE + 1];
        {
            for n in 0..KEY_SIZE {
                // XOR the key with the outer padding
                hash_in[n] = key[n] ^ HASH_OUTER_PAD;
                // XOR the key with the inner padding
                hash_out[n] = key[n] ^ HASH_INNER_PAD;
            }
            // Append the input byte
            hash_out[BLOCK_SIZE] = input;
            // Hash hash_out to form (Key XOR opad) || H((Key XOR ipad) || text)
            self.hash_key_hash(&hash_out[..=BLOCK_SIZE], &mut hash_in[BLOCK_SIZE..])?;
            // Hash hash_in to get the result
            self.hash_key_hash(&hash_in, &mut hash_out)?;
        }
        {
            // Take the key
            let (output_key, _) = result.split_at_mut(KEY_SIZE);
            output_key.copy_from_slice(&hash_out[..KEY_SIZE]);
        }
        Ok(())
    }
}

/// Default link key, "ZigBeeAlliance09"
pub const DEFAULT_LINK_KEY: [u8; KEY_SIZE] = [
    0x5a, 0x69, 0x67, 0x42, 0x65, 0x65, 0x41, 0x6c, 0x6c, 0x69, 0x61, 0x6e, 0x63, 0x65, 0x30, 0x39,
];

#[app(device = nrf52840_pac, peripherals = true)]
const APP: () = {
    struct Resources {
        security_service: SecurityService<CryptoCellBackend>,
    }

    #[init]
    fn init(cx: init::Context) -> init::LateResources {
        // Configure to use external clocks, and start them
        let _clocks = clocks::Clocks::new(cx.device.CLOCK)
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let cryptocell = CryptoCellBackend::new(cx.device.CRYPTOCELL);
        let security_service = SecurityService::new(cryptocell);

        init::LateResources { security_service }
    }

    #[idle(resources = [security_service])]
    fn idle(cx: idle::Context) -> ! {
        let security_service = cx.resources.security_service;

        let key = [
            0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD,
            0xCE, 0xCF,
        ];
        let nonce = [
            0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0x03, 0x02, 0x01, 0x00, 0x06,
        ];
        let message = [
            0x1A, 0x55, 0xA3, 0x6A, 0xBB, 0x6C, 0x61, 0x0D, 0x06, 0x6B, 0x33, 0x75, 0x64, 0x9C,
            0xEF, 0x10, 0xD4, 0x66, 0x4E, 0xCA, 0xD8, 0x54, 0xA8,
        ];
        let additional_data = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07];
        // M, length of the authentication field in octets 0, 4, 6, 8, 10, 12, 14, 16
        const MIC_LENGTH: usize = 8;
        let mut output = [0u8; 64];

        let mic = [0x0A, 0x89, 0x5C, 0xC1, 0xD8, 0xFF, 0x94, 0x69];

        match security_service.backend.ccmstar_decrypt(
            &key,
            &nonce,
            &message,
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
                    hprintln!("CCM Test 1 succeded").unwrap();
                } else {
                    hprintln!("CCM Test 1 failed, Mismatching output").unwrap();
                }
            }
            Err(e) => {
                hprintln!("CCM Test 1 failed").unwrap();
                if let nrf52_cryptocell::Error::Other(errno) = e {
                    hprintln!("CC Error {:08x}", errno).unwrap();
                }
            }
        }

        // C.6.1 Test Vector Set 1
        let key = [
            0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D,
            0x4E, 0x4F,
        ];
        let mut calculated = [0; BLOCK_SIZE];
        security_service
            .hash_key(&key, 0xc0, &mut calculated)
            .unwrap();
        if calculated
            == [
                0x45, 0x12, 0x80, 0x7B, 0xF9, 0x4C, 0xB3, 0x40, 0x0F, 0x0E, 0x2C, 0x25, 0xFB, 0x76,
                0xE9, 0x99,
            ]
        {
            hprintln!("AES ECB Test 1 succeded").unwrap();
        } else {
            hprintln!("AES ECB Test 1 failed").unwrap();
            for b in calculated.iter() {
                hprint!("{:02x}", b).unwrap();
            }
            hprintln!().unwrap();
        }

        security_service
            .hash_key(&DEFAULT_LINK_KEY, 0x00, &mut calculated)
            .unwrap();
        if calculated
            == [
                0x4b, 0xab, 0x0f, 0x17, 0x3e, 0x14, 0x34, 0xa2, 0xd5, 0x72, 0xe1, 0xc1, 0xef, 0x47,
                0x87, 0x82,
            ]
        {
            hprintln!("AES ECB Test 2 succeded").unwrap();
        } else {
            hprintln!("AES ECB Test 2 failed").unwrap();
            for b in calculated.iter() {
                hprint!("{:02x}", b).unwrap();
            }
            hprintln!().unwrap();
        }

        loop {
            continue;
        }
    }
};
