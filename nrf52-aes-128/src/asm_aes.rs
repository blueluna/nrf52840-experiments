//! Assambler from https://github.com/Ko-/aes-armcortexm
//!
//! Also interesting? https://github.com/jnk0le/cortexm-AES

extern "C" {
    fn AES_128_keyschedule(key: *const u8, rk: *mut u8);
    fn AES_128_keyschedule_dec(key: *const u8, rk: *mut u8);
    fn AES_128_encrypt(rk: *const u8, input: *const u8, output: *mut u8);
    fn AES_128_decrypt(rk: *const u8, input: *const u8, output: *mut u8);
}

pub use psila_crypto::{Error, BLOCK_SIZE, KEY_SIZE};

#[repr(align(4))]
pub struct AsmAes128Encrypt {
    rk: [u8; BLOCK_SIZE * 11],
}

impl AsmAes128Encrypt {
    pub fn new(key: &[u8]) -> Self {
        let mut rk = [0u8; BLOCK_SIZE * 11];
        rk[..KEY_SIZE].copy_from_slice(key);
        unsafe {
            AES_128_keyschedule(key.as_ptr(), rk[KEY_SIZE..].as_mut_ptr());
        }
        Self { rk }
    }
}

impl crate::Aes128Encrypt for AsmAes128Encrypt {
    /// Process blocks of data
    fn aes128_encrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        unsafe {
            AES_128_encrypt(self.rk.as_ptr(), input.as_ptr(), output.as_mut_ptr());
        }
    }
}

#[repr(align(4))]
pub struct AsmAes128Decrypt {
    rk: [u8; BLOCK_SIZE * 11],
}

impl AsmAes128Decrypt {
    pub fn new(key: &[u8]) -> Self {
        let mut rk = [0u8; BLOCK_SIZE * 11];
        rk[KEY_SIZE * 10..KEY_SIZE * 10 + KEY_SIZE].copy_from_slice(key);
        unsafe {
            AES_128_keyschedule_dec(key.as_ptr(), rk.as_mut_ptr());
        }
        Self { rk }
    }
}

impl crate::Aes128Decrypt for AsmAes128Decrypt {
    /// Process blocks of data
    fn aes128_decrypt(&mut self, input: &[u8], output: &mut [u8]) {
        unsafe {
            AES_128_decrypt(self.rk.as_ptr(), input.as_ptr(), output.as_mut_ptr());
        }
    }
}
