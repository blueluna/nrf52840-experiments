//! nRF52840 CryptoCell

#![no_std]

use nrf52840_pac::CRYPTOCELL;
pub use psila_crypto_trait::{CryptoBackend, Error};

/// Key length
pub const KEY_SIZE: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum EncryptDecrypt {
    /// Encryp operation
    Encrypt = 0,
    /// Decryp operation
    Decrypt = 1,
}

/// Block cipher key type
#[derive(Clone, Debug, PartialEq)]
pub enum KeyType {
    /// 128-bit AES key
    Aes128 = 0,
    /// 192-bit AES key
    Aes192 = 1,
    /// 256-bit AES key
    Aes256 = 2,
    /// 512-bit AES key
    Aes512 = 3,
}

/// Block cipher operation mode
#[derive(Clone, Debug, PartialEq)]
pub enum AesOperationMode {
    /// Electronic codebook
    Ecb = 0,
    /// Chiper block chaining
    Cbc = 1,
    /// Chiper block chaining - message authentication code
    CbcMac = 2,
    /// Counter
    Ctr = 3,
    /// Chiper block chaining - message authentication code with extras
    XCbcMac = 4,
    /// Cipher-based message authentication code
    CMac = 5,
    /// XEX with tweak and ciphertext stealing
    Xts = 6,
    /// Chiper block chaining with ciphertext stealing
    CbcCts = 7,
    /// Output feed-back
    Ofb = 8,
}

/// Padding type
#[derive(Clone, Debug, PartialEq)]
pub enum PaddingType {
    /// None, padded with zeroes
    None = 0,
    /// PKCS7 padding
    Pkcs7 = 1,
}

/// CCM-mode
#[derive(Clone, Debug, PartialEq)]
pub enum CcmMode {
    /// CCM
    Ccm = 0,
    /// CCM*
    CcmStar = 1,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CryptoCellAesContext {
    buff: [u32; 19usize],
}

extern "C" {
    /// Initialize CryptoCell runtime library
    fn SaSi_LibInit() -> u32;
    /// Finalize library operations
    fn SaSi_LibFini();
    /// Perform AES-CCM
    fn CC_AESCCM(
        // Encrypt (0) or decrypt (1)
        decrypt: u32,
        // The key to use
        key: *const u8,
        // Key type used
        //   0 - 128-bit AES key
        //   1 - 192-bit AES key
        //   2 - 256-bit AES key
        //   3 - 512-bit AES key
        keySize: u32,
        // Nonce to use
        nonce: *const u8,
        // Size of the nonce
        nonceSize: u8,
        // Additional data to use
        aad: *const u8,
        // Size of the additional data
        aadSize: u32,
        // Data to process
        dataIn: *const u8,
        // Size of data to process
        dataInSize: u32,
        // Output of processed data, shall be at least the same size as `dataIn`.
        dataOut: *mut u8,
        // Size of the message integrity code (MIC)
        micSize: u8,
        // The message integrity code (MIC)
        mic: *mut u8,
        // CMM mode, either CMM (0) or CMM* (1)
        cmmMode: u32,
    ) -> u32;
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
    fn SaSi_AesSetIv(context: *mut CryptoCellAesContext, iv: *const u8) -> u32;
    fn SaSi_AesGetIv(context: *mut CryptoCellAesContext, iv: *mut u8) -> u32;
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
    /// Finalize a cipher operation
    fn SaSi_AesFinish(
        // AES context
        context: *mut CryptoCellAesContext,
        // Size of data to process
        dataInSize: usize,
        // Data to process
        dataIn: *const u8,
        // Size of data buffer provided
        dataInBufferSize: usize,
        // Pointer to output data
        dataOut: *mut u8,
        // Size of output buffer provided
        DataOutBufferSize: *mut usize,
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
    pub key: *const u8,
    pub size: usize,
}

pub struct AesContext {
    context: CryptoCellAesContext,
}

impl AesContext {
    fn new(encrypt: EncryptDecrypt, mode: AesOperationMode, padding_type: PaddingType) -> Self {
        let mut context = CryptoCellAesContext { buff: [0u32; 19] };
        let ctx_ptr = &mut context as *mut CryptoCellAesContext;
        let result =
            unsafe { SaSi_AesInit(ctx_ptr, encrypt as u32, mode as u32, padding_type as u32) };
        if result != 0 {
            panic!("Failed to initialize AES context");
        }

        Self { context }
    }

    fn context(&mut self) -> *mut CryptoCellAesContext {
        &mut self.context as *mut CryptoCellAesContext
    }

    fn set_key(&mut self, key: &[u8]) -> Result<(), Error> {
        assert!(key.len() == KEY_SIZE);
        let user_key = KeyData {
            key: key.as_ptr(),
            size: key.len(),
        };
        let result = unsafe {
            SaSi_AesSetKey(
                self.context(),
                0, // User key
                &user_key as *const KeyData as *const cty::c_void,
                core::mem::size_of::<KeyData>(),
            )
        };
        if result != 0 {
            return Err(Error::Other(result));
        }
        Ok(())
    }

    /// Set the IV
    fn set_iv(&mut self, iv: &[u8]) -> Result<(), Error> {
        assert!(iv.len() == 16);
        let result = unsafe { SaSi_AesSetIv(self.context(), iv.as_ptr()) };
        if result != 0 {
            return Err(Error::Other(result));
        }
        Ok(())
    }

    /// Set the IV
    fn get_iv(&mut self, iv: &mut [u8]) -> Result<(), Error> {
        assert!(iv.len() == 16);
        let result = unsafe { SaSi_AesGetIv(self.context(), iv.as_mut_ptr()) };
        if result != 0 {
            return Err(Error::Other(result));
        }
        Ok(())
    }

    fn process_block(&mut self, input: &[u8], output: &mut [u8]) -> Result<(), Error> {
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
            return Err(Error::Other(result));
        }
        Ok(())
    }

    fn finish(&mut self, input: &[u8], output: &mut [u8]) -> Result<(), Error> {
        assert!(input.len() <= output.len());
        assert!(input.len() == 16);
        let mut output_length = output.len();
        let result = unsafe {
            SaSi_AesFinish(
                self.context(),
                16,
                input.as_ptr(),
                16,
                output.as_mut_ptr(),
                (&mut output_length) as *mut usize,
            )
        };
        if result != 0 {
            return Err(Error::Other(result));
        }
        Ok(())
    }
}

impl core::ops::Drop for AesContext {
    fn drop(&mut self) {
        unsafe { SaSi_AesFree(self.context()) };
    }
}

pub struct CryptoCellBackend {
    cryptocell: CRYPTOCELL,
    cipher: AesContext,
}

impl core::ops::Drop for CryptoCellBackend {
    fn drop(&mut self) {
        unsafe { SaSi_LibFini() }
        self.cryptocell.enable.write(|w| w.enable().clear_bit());
    }
}

impl CryptoCellBackend {
    pub fn new(cryptocell: CRYPTOCELL) -> Self {
        cryptocell.enable.write(|w| w.enable().set_bit());

        if unsafe { SaSi_LibInit() } != 0 {
            panic!("Failed to initialize SaSi library");
        }

        let cipher = AesContext::new(
            EncryptDecrypt::Encrypt,
            AesOperationMode::Ecb,
            PaddingType::None,
        );

        Self { cryptocell, cipher }
    }
}

impl CryptoBackend for CryptoCellBackend {
    fn ccmstar_decrypt(
        &mut self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        mic_length: usize,
        additional_data: &[u8],
        message_output: &mut [u8],
    ) -> Result<usize, Error> {
        let message_len = (message.len() - mic_length) as u32;
        let mut mic_result = [0u8; 16];
        mic_result[..mic_length].copy_from_slice(&message[message_len as usize..]);
        let result = unsafe {
            CC_AESCCM(
                EncryptDecrypt::Decrypt as u32,
                key.as_ptr(),
                KeyType::Aes128 as u32,
                nonce.as_ptr(),
                nonce.len() as u8,
                additional_data.as_ptr(),
                additional_data.len() as u32,
                message.as_ptr(),
                message_len,
                message_output.as_mut_ptr(),
                mic_length as u8,
                mic_result.as_mut_ptr(),
                CcmMode::CcmStar as u32,
            )
        };
        if result != 0 {
            return Err(Error::Other(result));
        }
        Ok(0)
    }

    fn ccmstar_encrypt(
        &mut self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        mic_length: usize,
        additional_data: &[u8],
        message_output: &mut [u8],
    ) -> Result<usize, Error> {
        let message_len = (message.len() - mic_length) as u32;
        let mut mic_result = [0u8; 16];
        mic_result[..mic_length].copy_from_slice(&message[message_len as usize..]);
        let result = unsafe {
            CC_AESCCM(
                EncryptDecrypt::Decrypt as u32,
                key.as_ptr(),
                KeyType::Aes128 as u32,
                nonce.as_ptr(),
                nonce.len() as u8,
                additional_data.as_ptr(),
                additional_data.len() as u32,
                message.as_ptr(),
                message_len,
                message_output.as_mut_ptr(),
                mic_length as u8,
                mic_result.as_mut_ptr(),
                CcmMode::CcmStar as u32,
            )
        };
        if result != 0 {
            return Err(Error::Other(result));
        }
        Ok(0)
    }

    /// Set the key
    fn aes128_ecb_encrypt_set_key(&mut self, key: &[u8]) -> Result<(), Error> {
        self.cipher.set_key(key)
    }
    /// Set the IV
    fn aes128_ecb_encrypt_set_iv(&mut self, iv: &[u8]) -> Result<(), Error> {
        self.cipher.set_iv(iv)
    }
    /// Process blocks of data
    fn aes128_ecb_encrypt_process_block(
        &mut self,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<(), Error> {
        self.cipher.process_block(input, output)
    }
    /// Process the last bits and bobs and finish
    fn aes128_ecb_encrypt_finish(&mut self, input: &[u8], output: &mut [u8]) -> Result<(), Error> {
        self.cipher.finish(input, output)
    }
}
