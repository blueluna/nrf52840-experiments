use aes::{
    block_cipher_trait::generic_array::GenericArray, block_cipher_trait::BlockCipher, Aes128,
};

pub use psila_crypto::{Error, BLOCK_SIZE, KEY_SIZE};

pub struct RustAes128Encrypt {
    aes: Aes128,
}

impl RustAes128Encrypt {
    pub fn new(key: &[u8]) -> Self {
        Self {
            aes: Aes128::new(GenericArray::from_slice(key)),
        }
    }
}

impl crate::Aes128Encrypt for RustAes128Encrypt {
    fn aes128_encrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        output.copy_from_slice(input);
        self.aes.encrypt_block(GenericArray::from_mut_slice(output));
    }
}

pub struct RustAes128Decrypt {
    aes: Aes128,
}

impl RustAes128Decrypt {
    pub fn new(key: &[u8]) -> Self {
        Self {
            aes: Aes128::new(GenericArray::from_slice(key)),
        }
    }
}

impl crate::Aes128Decrypt for RustAes128Decrypt {
    fn aes128_decrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        output.copy_from_slice(input);
        self.aes.decrypt_block(GenericArray::from_mut_slice(output));
    }
}
