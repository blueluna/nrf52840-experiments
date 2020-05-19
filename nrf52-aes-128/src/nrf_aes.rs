use psila_crypto::BLOCK_SIZE;

use crate::{ecb::Ecb, Aes128Encrypt};

pub struct NrfAes128Encrypt<'a> {
    ecb: &'a mut Ecb,
}

impl<'a> NrfAes128Encrypt<'a> {
    pub fn new(ecb: &'a mut Ecb, key: &[u8]) -> Self {
        ecb.key().copy_from_slice(key);
        Self { ecb }
    }
}

impl<'a> Aes128Encrypt for NrfAes128Encrypt<'a> {
    fn aes128_encrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        self.ecb.clear_text().copy_from_slice(&input);
        match self.ecb.encrypt() {
            Ok(_) => {}
            Err(_) => {
                panic!();
            }
        }
        output.copy_from_slice(self.ecb.cipher_text());
    }
}
