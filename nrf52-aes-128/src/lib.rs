//! # nRF52 AES128 ECB hardware block
//!
//! Functions for using the AES128 ECB hardware block in the nRF52 for the
//! security in Psila.
//!

#![no_std]

use byteorder::{BigEndian, ByteOrder};
use psila_crypto::{BLOCK_SIZE, KEY_SIZE};

mod asm_aes;
mod cryptocell;
mod ecb;
mod nrf_aes;
mod rust_aes;

pub use asm_aes::{AsmAes128Decrypt, AsmAes128Encrypt};
pub use cryptocell::{
    cryptocell_init, CryptoCellAes128CbcDecrypt, CryptoCellAes128CbcEncrypt,
    CryptoCellAes128Decrypt, CryptoCellAes128Encrypt,
};
pub use ecb::{Ecb, EcbData};
pub use nrf_aes::NrfAes128Encrypt;
pub use rust_aes::{RustAes128Decrypt, RustAes128Encrypt};

pub trait Aes128Encrypt {
    fn aes128_encrypt(&mut self, input: &[u8], output: &mut [u8]);
}
pub trait Aes128Decrypt {
    fn aes128_decrypt(&mut self, input: &[u8], output: &mut [u8]);
}

#[inline(always)]
fn block_xor(a: &[u8], b: &[u8], r: &mut [u8]) {
    let a32 = BigEndian::read_u32(&a[0..4]);
    let b32 = BigEndian::read_u32(&b[0..4]);
    BigEndian::write_u32(&mut r[0..4], a32 ^ b32);

    let a32 = BigEndian::read_u32(&a[4..8]);
    let b32 = BigEndian::read_u32(&b[4..8]);
    BigEndian::write_u32(&mut r[4..8], a32 ^ b32);

    let a32 = BigEndian::read_u32(&a[8..12]);
    let b32 = BigEndian::read_u32(&b[8..12]);
    BigEndian::write_u32(&mut r[8..12], a32 ^ b32);

    let a32 = BigEndian::read_u32(&a[12..16]);
    let b32 = BigEndian::read_u32(&b[12..16]);
    BigEndian::write_u32(&mut r[12..16], a32 ^ b32);
}

#[inline(always)]
fn block_xor_in_place(a: &[u8], r: &mut [u8]) {
    let a32 = BigEndian::read_u32(&a[0..4]);
    let r32 = BigEndian::read_u32(&r[0..4]);
    BigEndian::write_u32(&mut r[0..4], a32 ^ r32);

    let a32 = BigEndian::read_u32(&a[4..8]);
    let r32 = BigEndian::read_u32(&r[4..8]);
    BigEndian::write_u32(&mut r[4..8], a32 ^ r32);

    let a32 = BigEndian::read_u32(&a[8..12]);
    let r32 = BigEndian::read_u32(&r[8..12]);
    BigEndian::write_u32(&mut r[8..12], a32 ^ r32);

    let a32 = BigEndian::read_u32(&a[12..16]);
    let r32 = BigEndian::read_u32(&r[12..16]);
    BigEndian::write_u32(&mut r[12..16], a32 ^ r32);
}

#[inline(always)]
fn counter_inc(counter: &mut [u8]) {
    let mut counter32_1 = BigEndian::read_u32(&counter[12..16]);
    let mut counter32_2 = BigEndian::read_u32(&counter[8..12]);
    let mut counter32_3 = BigEndian::read_u32(&counter[4..8]);
    let mut counter32_4 = BigEndian::read_u32(&counter[0..4]);
    if counter32_1 == 0xffff {
        if counter32_2 == 0xffff {
            if counter32_3 == 0xffff {
                counter32_4 = counter32_4.wrapping_add(1);
            }
            counter32_3 = counter32_3.wrapping_add(1);
        }
        counter32_2 = counter32_2.wrapping_add(1);
    }
    counter32_1 = counter32_1.wrapping_add(1);
    BigEndian::write_u32(&mut counter[12..16], counter32_1);
    BigEndian::write_u32(&mut counter[8..12], counter32_2);
    BigEndian::write_u32(&mut counter[4..8], counter32_3);
    BigEndian::write_u32(&mut counter[0..4], counter32_4);
}

#[repr(align(4))]
pub struct Aes128CtrEncrypt<'a, T: Aes128Encrypt> {
    cipher: &'a mut T,
    counter: [u8; BLOCK_SIZE],
    block: [u8; BLOCK_SIZE],
}

impl<'a, T: Aes128Encrypt> Aes128CtrEncrypt<'a, T> {
    pub fn new(cipher: &'a mut T, counter: [u8; BLOCK_SIZE]) -> Self {
        Self {
            cipher,
            counter,
            block: [0; BLOCK_SIZE],
        }
    }
    /// Process blocks of data
    pub fn encrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        self.cipher.aes128_encrypt(&self.counter, &mut self.block);
        block_xor(input, &self.block, output);
        counter_inc(&mut self.counter);
    }
}

#[repr(align(4))]
pub struct Aes128CtrDecrypt<'a, T: Aes128Encrypt> {
    cipher: &'a mut T,
    counter: [u8; BLOCK_SIZE],
    block: [u8; BLOCK_SIZE],
}

impl<'a, T: Aes128Encrypt> Aes128CtrDecrypt<'a, T> {
    pub fn new(cipher: &'a mut T, counter: [u8; BLOCK_SIZE]) -> Self {
        Self {
            cipher,
            counter,
            block: [0; BLOCK_SIZE],
        }
    }
    /// Process blocks of data
    pub fn decrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        self.cipher.aes128_encrypt(&self.counter, &mut self.block);
        block_xor(input, &self.block, output);
        // WARNING: Only updating 1/4 of the counter!
        let mut counter = BigEndian::read_u32(&self.counter[12..16]);
        counter = counter.wrapping_add(1);
        BigEndian::write_u32(&mut self.counter[12..16], counter);
    }
}

#[repr(align(4))]
pub struct Aes128CbcEncrypt<'a, T: Aes128Encrypt> {
    cipher: &'a mut T,
    block: [u8; BLOCK_SIZE],
}

impl<'a, T: Aes128Encrypt> Aes128CbcEncrypt<'a, T> {
    pub fn new(cipher: &'a mut T, initialisation_vector: [u8; BLOCK_SIZE]) -> Self {
        Self {
            cipher,
            block: initialisation_vector,
        }
    }
    /// Process blocks of data
    pub fn encrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        block_xor_in_place(input, &mut self.block[..]);
        self.cipher.aes128_encrypt(&self.block, output);
        self.block.copy_from_slice(output);
    }
}

#[repr(align(4))]
pub struct Aes128CbcDecrypt<'a, T: Aes128Decrypt> {
    cipher: &'a mut T,
    block: [u8; BLOCK_SIZE],
}

impl<'a, T: Aes128Decrypt> Aes128CbcDecrypt<'a, T> {
    pub fn new(cipher: &'a mut T, initialisation_vector: [u8; BLOCK_SIZE]) -> Self {
        Self {
            cipher,
            block: initialisation_vector,
        }
    }
    /// Process blocks of data
    pub fn decrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() == BLOCK_SIZE);
        assert!(output.len() == BLOCK_SIZE);
        self.cipher.aes128_decrypt(input, output);
        block_xor_in_place(&self.block[..], output);
        self.block.copy_from_slice(input);
    }
}
