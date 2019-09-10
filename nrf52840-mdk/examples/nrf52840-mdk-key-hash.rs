#![no_main]
#![no_std]

#[allow(unused_imports)]
use panic_semihosting;

use cortex_m_semihosting::hprintln;
use rtfm::app;

use nrf52840_hal::{clocks, prelude::*};

use nrf52840_pac as pac;

/// Key length
pub const KEY_SIZE: usize = 16;

/// Cipher block length
pub const BLOCK_SIZE: usize = 16;

pub const ECB_BLOCK_SIZE: usize = KEY_SIZE + BLOCK_SIZE + BLOCK_SIZE;


#[derive(Clone, Debug, PartialEq)]
pub enum SecurityError {
    ResourceConflict,
}

pub struct Aes128Ecb {
    ecb: pac::ECB,
    buffer: [u8; ECB_BLOCK_SIZE],
}

impl Aes128Ecb {
    pub fn new(ecb: pac::ECB) -> Self {
        Self {
            ecb,
            buffer: [0u8; ECB_BLOCK_SIZE],
        }
    }

    pub fn set_key(&mut self, key: &[u8]) -> Result<(), SecurityError>
    {
        assert!(key.len() == KEY_SIZE);
        self.buffer[..KEY_SIZE].copy_from_slice(&key);
        Ok(())
    }

    pub fn process(&mut self, input: &[u8], output: &mut [u8]) -> Result<(), SecurityError>
    {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        self.buffer[KEY_SIZE..KEY_SIZE + BLOCK_SIZE].copy_from_slice(input);
        let data_ptr = &mut self.buffer as *mut _ as u32;
        self.ecb.ecbdataptr.write(|w| unsafe { w.bits(data_ptr) });
        self.ecb.tasks_startecb.write(|w| w.tasks_startecb().set_bit());
        loop {
            if self.ecb.events_errorecb.read().events_errorecb().bit_is_set() {
                return Err(SecurityError::ResourceConflict);
            }
            if self.ecb.events_endecb.read().events_endecb().bit_is_set() {
                output.copy_from_slice(&self.buffer[KEY_SIZE + BLOCK_SIZE..]);
                break;
            }
        }
        Ok(())
    }
}

pub struct SecurityService {
    cipher: Aes128Ecb,
}

impl SecurityService {
    pub fn new(cipher: Aes128Ecb) -> Self {
        Self {
            cipher,
        }
    }

    fn hash_process_block(&mut self, input: &[u8], mut output: &mut [u8], ) -> Result<(), SecurityError> {
        self.cipher.set_key(output)?;
        self.cipher.process(input, &mut output)?;
        /* Now we have to XOR the input into the hash block. */
        for n in 0..BLOCK_SIZE {
            output[n] ^= input[n];
        }
        Ok(())
    }


    pub fn hash(&mut self, input: &[u8], output: &mut [u8]) -> Result<(), SecurityError> {
        /* Cipher Instance. */
        let mut block = [0u8; BLOCK_SIZE];

        /* Clear the first hash block (Hash0). */
        for output_byte in &mut output[..BLOCK_SIZE] {
            *output_byte = 0;
        }

        /* Create the subsequent hash blocks using the formula: Hash[i] = E(Hash[i-1], M[i]) XOR M[i]
         *
         * because we can't guarantee that M will be exactly a multiple of the
         * block size, we will need to copy it into local buffers and pad it.
         *
         * Note that we check for the next cipher block at the end of the loop
         * rather than the start. This is so that if the input happens to end
         * on a block boundary, the next cipher block will be generated for the
         * start of the padding to be placed into.
         */
        let mut j = 0;
        for input_byte in input.iter() {
            /* Copy data into the cipher input. */
            block[j] = *input_byte;
            j += 1;
            /* Check if this cipher block is done. */
            if j >= BLOCK_SIZE {
                /* We have reached the end of this block. Process it with the
                 * cipher, note that the Key input to the cipher is actually
                 * the previous hash block, which we are keeping in output.
                 */
                self.hash_process_block(&block, &mut output[0..BLOCK_SIZE])?;
                /* Reset j to start again at the beginning at the next block. */
                j = 0;
            }
        }
        /* Need to append the bit '1', followed by '0' padding long enough to end
         * the hash input on a block boundary. However, because 'n' is 16, and 'l'
         * will be a multiple of 8, the padding will be >= 7-bits, and we can just
         * append the byte 0x80.
         */
        block[j] = 0x80;
        j += 1;
        /* Pad with '0' until the the current block is exactly 'n' bits from the
         * end.
         */
        while j != (BLOCK_SIZE - 2) {
            if j >= BLOCK_SIZE {
                /* We have reached the end of this block. Process it with the
                 * cipher, note that the Key input to the cipher is actually
                 * the previous hash block, which we are keeping in output.
                 */
                self.hash_process_block(&block, &mut output[0..BLOCK_SIZE])?;
                /* Reset j to start again at the beginning at the next block. */
                j = 0;
            }
            /* Pad the input with 0. */
            block[j] = 0x00;
            j += 1;
        }
        let input_len = input.len() as u16 * 8;
        /* Add the 'n'-bit representation of 'l' to the end of the block. */
        block[j] = (input_len >> 8) as u8;
        j += 1;
        block[j] = (input_len & 0xff) as u8;
        /* Process the last cipher block. */
        self.hash_process_block(&block, &mut output[0..BLOCK_SIZE])?;
        /* Cleanup the cipher. */
        /* Done */
        Ok(())
    }

    pub fn keyed_hash(&mut self, key: &[u8; KEY_SIZE], input: u8, result: &mut [u8]) -> Result<(), SecurityError> {
        const HASH_INNER_PAD: u8 = 0x36;
        const HASH_OUTER_PAD: u8 = 0x5c;
        let mut hash_in = [0; BLOCK_SIZE * 2];
        let mut hash_out = [0; BLOCK_SIZE + 1];

        {
            /* Copy the key into hash_in and XOR with opad to form: (Key XOR opad) */
            for n in 0..KEY_SIZE {
                hash_in[n] = key[n] ^ HASH_OUTER_PAD;
            }

            /* Copy the Key into hash_out and XOR with ipad to form: (Key XOR ipad) */
            for n in 0..KEY_SIZE {
                hash_out[n] = key[n] ^ HASH_INNER_PAD;
            }

            /* Append the input byte to form: (Key XOR ipad) || text. */
            hash_out[BLOCK_SIZE] = input;

            /* Hash the contents of hash_out and append the contents to hash_in to
             * form: (Key XOR opad) || H((Key XOR ipad) || text).
             */
            self.hash(&hash_out[..=BLOCK_SIZE], &mut hash_in[BLOCK_SIZE..])?;

            /* Hash the contents of hash_in to get the final result. */
            self.hash(&hash_in, &mut hash_out)?;
        }

        {
            let (output_key, _) = result.split_at_mut(KEY_SIZE);
            output_key.copy_from_slice(&hash_out[..KEY_SIZE]);
        }

        Ok(())
    }
}


#[app(device = nrf52840_pac)]
const APP: () = {
    static mut SECURITY_SERVICE: SecurityService = ();

    #[init]
    fn init() {
        // Configure to use external clocks, and start them
        let _clocks = device
            .CLOCK
            .constrain()
            .enable_ext_hfosc()
            .set_lfclk_src_external(clocks::LfOscConfiguration::NoExternalNoBypass)
            .start_lfclk();

        let aes128_ecb = Aes128Ecb::new(device.ECB);
        let security_service = SecurityService::new(aes128_ecb);

        SECURITY_SERVICE = security_service;
    }

    #[idle(resources = [SECURITY_SERVICE])]
    fn idle() -> ! {
        let mut security_service = resources.SECURITY_SERVICE;

        // C.6.1 Test Vector Set 1
        let key = [
            0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D,
            0x4E, 0x4F,
        ];
        let mut calculated = [0; BLOCK_SIZE];
        security_service.keyed_hash(&key, 0xc0, &mut calculated).unwrap();
        if calculated ==  [ 0x45, 0x12, 0x80, 0x7B, 0xF9, 0x4C, 0xB3, 0x40, 0x0F, 0x0E, 0x2C, 0x25, 0xFB, 0x76, 0xE9, 0x99 ] {
            hprintln!("Test succeded").unwrap();
        }
        else {
            hprintln!("Test failed").unwrap();
        }
        loop {}
    }
};
