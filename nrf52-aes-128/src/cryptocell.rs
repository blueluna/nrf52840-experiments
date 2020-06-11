//! # nRF52840 CryptoCell for Psila
//!
//! Functions for using the CryptoCell in the nRF52840 for the security in
//! Psila. This uses the nrf_cc310 library provided by Nordic in their SDK.
//!
//! Note that there is a CCM* implementation in the CC310 library provided by
//! Nordic. But that implementation requires the last byte of the nonce to
//! only indicate the MIC length and encryption. For the usage in Psila this
//! byte contains more bits of information.

use crate::{BLOCK_SIZE, KEY_SIZE};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CryptoCellAesContext {
    buffer: [u32; 19usize],
}

extern "C" {
    /// Initialize CryptoCell runtime library
    fn SaSi_LibInit() -> u32;
    /// Initialize AES context
    fn SaSi_AesInit(
        // The context to initalize
        context: *mut CryptoCellAesContext,
        // Encrypt (0) or decrypt (1)
        decrypt: u32,
        // AES operation mode,
        //   0 - ECB
        //   1 - CBC
        //   2 - CBC-MAC
        //   3 - CTR
        //   4 - XCBC-MAC
        //   5 - CMAC
        //   6 - XTS
        //   7 - CBC-CTS
        //   8 - OFB
        operationMode: u32,
        // Padding type, None (0) or PKCS7 (1)
        paddingType: u32,
    ) -> u32;
    /// Set a key for the AES context
    fn SaSi_AesSetKey(
        // AES context
        context: *mut CryptoCellAesContext,
        // Key type used
        //   0 - User key
        //   1 - Platform key
        //   2 - Customer key
        keyType: u32,
        // Pointer to key struct
        keyData: *const cty::c_void,
        // Size of the key struct
        keyDataSize: usize,
    ) -> u32;
    /// Set IV (or counter) for the AES context
    fn SaSi_AesSetIv(context: *mut CryptoCellAesContext, iv: *const u8) -> u32;
    /// Process a block of data
    fn SaSi_AesBlock(
        // AES context
        context: *mut CryptoCellAesContext,
        // Data to process
        dataIn: *const u8,
        // Size of data to process
        dataInSize: usize,
        // Pointer to output data
        dataOut: *mut u8,
    ) -> u32;
    /// Clean up a AES context
    fn SaSi_AesFree(
        // AES context
        context: *mut CryptoCellAesContext,
    ) -> u32;
}

/// CryptoCell Key Data
#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct KeyData {
    /// Key data
    pub key: *const u8,
    /// Key length
    pub size: usize,
}

pub fn cryptocell_init() {
    if unsafe { SaSi_LibInit() } != 0 {
        panic!("Failed to initialize SaSi library");
    }
}

pub struct CryptoCellAes128Encrypt {
    context: CryptoCellAesContext,
}

impl CryptoCellAes128Encrypt {
    pub fn new(key: &[u8]) -> Self {
        let mut context = CryptoCellAesContext { buffer: [0u32; 19] };
        let ctx_ptr = &mut context as *mut CryptoCellAesContext;
        let result = unsafe { SaSi_AesInit(ctx_ptr, 0, 0, 0) };
        if result != 0 {
            panic!("Failed to initialize AES context {:08x}", result);
        }
        assert!(key.len() == KEY_SIZE);
        let user_key = KeyData {
            key: key.as_ptr(),
            size: key.len(),
        };
        let result = unsafe {
            SaSi_AesSetKey(
                ctx_ptr,
                0, // User key
                &user_key as *const KeyData as *const cty::c_void,
                core::mem::size_of::<KeyData>(),
            )
        };
        if result != 0 {
            panic!("Failed to set key {:08x}", result);
        }
        Self { context }
    }

    fn context(&mut self) -> *mut CryptoCellAesContext {
        &mut self.context as *mut CryptoCellAesContext
    }
}

impl core::ops::Drop for CryptoCellAes128Encrypt {
    fn drop(&mut self) {
        unsafe { SaSi_AesFree(self.context()) };
    }
}

impl crate::Aes128Encrypt for CryptoCellAes128Encrypt {
    /// Process a block of data
    fn aes128_encrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() <= output.len());
        assert!(input.len() <= 65535);
        let result = unsafe {
            SaSi_AesBlock(
                self.context(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
            )
        };
        if result != 0 {
            panic!("Failed to process block {:08x}", result);
        }
    }
}

pub struct CryptoCellAes128Decrypt {
    context: CryptoCellAesContext,
}

impl CryptoCellAes128Decrypt {
    pub fn new(key: &[u8]) -> Self {
        let mut context = CryptoCellAesContext { buffer: [0u32; 19] };
        let ctx_ptr = &mut context as *mut CryptoCellAesContext;
        let result = unsafe { SaSi_AesInit(ctx_ptr, 1, 0, 0) };
        if result != 0 {
            panic!("Failed to initialize AES context {:08x}", result);
        }
        assert!(key.len() == KEY_SIZE);
        let user_key = KeyData {
            key: key.as_ptr(),
            size: key.len(),
        };
        let result = unsafe {
            SaSi_AesSetKey(
                ctx_ptr,
                0, // User key
                &user_key as *const KeyData as *const cty::c_void,
                core::mem::size_of::<KeyData>(),
            )
        };
        if result != 0 {
            panic!("Failed to set key {:08x}", result);
        }
        Self { context }
    }

    fn context(&mut self) -> *mut CryptoCellAesContext {
        &mut self.context as *mut CryptoCellAesContext
    }
}

impl core::ops::Drop for CryptoCellAes128Decrypt {
    fn drop(&mut self) {
        unsafe { SaSi_AesFree(self.context()) };
    }
}

impl crate::Aes128Decrypt for CryptoCellAes128Decrypt {
    /// Process a block of data
    fn aes128_decrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() <= output.len());
        assert!(input.len() <= 65535);
        let result = unsafe {
            SaSi_AesBlock(
                self.context(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
            )
        };
        if result != 0 {
            panic!("Failed to process block {:08x}", result);
        }
    }
}

pub struct CryptoCellAes128CbcEncrypt {
    context: CryptoCellAesContext,
}

impl CryptoCellAes128CbcEncrypt {
    pub fn new(key: &[u8], initialisation_vector: [u8; BLOCK_SIZE]) -> Self {
        let mut context = CryptoCellAesContext { buffer: [0u32; 19] };
        let ctx_ptr = &mut context as *mut CryptoCellAesContext;
        let result = unsafe { SaSi_AesInit(ctx_ptr, 0, 1, 0) };
        if result != 0 {
            panic!("Failed to initialize AES context {:08x}", result);
        }
        assert!(key.len() == KEY_SIZE);
        let user_key = KeyData {
            key: key.as_ptr(),
            size: key.len(),
        };
        let result = unsafe {
            SaSi_AesSetKey(
                ctx_ptr,
                0, // User key
                &user_key as *const KeyData as *const cty::c_void,
                core::mem::size_of::<KeyData>(),
            )
        };
        if result != 0 {
            panic!("Failed to set key {:08x}", result);
        }
        let result = unsafe {
            SaSi_AesSetIv(ctx_ptr, initialisation_vector.as_ptr());
            SaSi_AesSetKey(
                ctx_ptr,
                0, // User key
                &user_key as *const KeyData as *const cty::c_void,
                core::mem::size_of::<KeyData>(),
            )
        };
        if result != 0 {
            panic!("Failed to set IV {:08x}", result);
        }
        Self { context }
    }

    fn context(&mut self) -> *mut CryptoCellAesContext {
        &mut self.context as *mut CryptoCellAesContext
    }
    /// Process a block of data
    pub fn encrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() <= output.len());
        assert!(input.len() <= 65535);
        let result = unsafe {
            SaSi_AesBlock(
                self.context(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
            )
        };
        if result != 0 {
            panic!("Failed to process block {:08x}", result);
        }
    }
}

impl core::ops::Drop for CryptoCellAes128CbcEncrypt {
    fn drop(&mut self) {
        unsafe { SaSi_AesFree(self.context()) };
    }
}

pub struct CryptoCellAes128CbcDecrypt {
    context: CryptoCellAesContext,
}

impl CryptoCellAes128CbcDecrypt {
    pub fn new(key: &[u8], initialisation_vector: [u8; BLOCK_SIZE]) -> Self {
        let mut context = CryptoCellAesContext { buffer: [0u32; 19] };
        let ctx_ptr = &mut context as *mut CryptoCellAesContext;
        let result = unsafe { SaSi_AesInit(ctx_ptr, 1, 1, 0) };
        if result != 0 {
            panic!("Failed to initialize AES context {:08x}", result);
        }
        assert!(key.len() == KEY_SIZE);
        let user_key = KeyData {
            key: key.as_ptr(),
            size: key.len(),
        };
        let result = unsafe {
            SaSi_AesSetKey(
                ctx_ptr,
                0, // User key
                &user_key as *const KeyData as *const cty::c_void,
                core::mem::size_of::<KeyData>(),
            )
        };
        if result != 0 {
            panic!("Failed to set key {:08x}", result);
        }
        let result = unsafe {
            SaSi_AesSetIv(ctx_ptr, initialisation_vector.as_ptr());
            SaSi_AesSetKey(
                ctx_ptr,
                0, // User key
                &user_key as *const KeyData as *const cty::c_void,
                core::mem::size_of::<KeyData>(),
            )
        };
        if result != 0 {
            panic!("Failed to set IV {:08x}", result);
        }
        Self { context }
    }

    fn context(&mut self) -> *mut CryptoCellAesContext {
        &mut self.context as *mut CryptoCellAesContext
    }
    /// Process a block of data
    pub fn decrypt(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(input.len() <= output.len());
        assert!(input.len() <= 65535);
        let result = unsafe {
            SaSi_AesBlock(
                self.context(),
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
            )
        };
        if result != 0 {
            panic!("Failed to process block {:08x}", result);
        }
    }
}

impl core::ops::Drop for CryptoCellAes128CbcDecrypt {
    fn drop(&mut self) {
        unsafe { SaSi_AesFree(self.context()) };
    }
}
