//! # nRF52840 CryptoCell for Psila
//!
//! Functions for using the CryptoCell in the nRF52840 for the security in
//! Psila. This uses the nrf_cc310 library provided by Nordic in their SDK.
//!
//! Note that there is a CCM* implementation in the CC310 library provided by
//! Nordic. But that implementation requires the last byte of the nonce to
//! only indicate the MIC length and encryption. For the usage in Psila this
//! byte contains more bits of information.

#![no_std]

use nrf52840_pac::CRYPTOCELL;
pub use psila_crypto::{
    BlockCipher, CryptoBackend, Error, BLOCK_SIZE, KEY_SIZE, LENGTH_FIELD_LENGTH,
};

fn clear(slice: &mut [u8]) {
    for v in slice.iter_mut() {
        *v = 0;
    }
}

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
    /// Get IV (or counter) for the AES context
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
    /// Key data
    pub key: *const u8,
    /// Key length
    pub size: usize,
}

/// CryptoCell AES context
pub struct AesContext {
    context: CryptoCellAesContext,
}

impl AesContext {
    pub fn new(encrypt: EncryptDecrypt, mode: AesOperationMode, padding_type: PaddingType) -> Self {
        let mut context = CryptoCellAesContext { buff: [0u32; 19] };
        let ctx_ptr = &mut context as *mut CryptoCellAesContext;
        let result =
            unsafe { SaSi_AesInit(ctx_ptr, encrypt as u32, mode as u32, padding_type as u32) };
        if result != 0 {
            panic!("Failed to initialize AES context {:08x}", result);
        }

        Self { context }
    }

    fn context(&mut self) -> *mut CryptoCellAesContext {
        &mut self.context as *mut CryptoCellAesContext
    }
}

impl BlockCipher for AesContext {
    /// Set the key to be used in the cipher operation
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

    /// Get the IV
    fn get_iv(&mut self, iv: &mut [u8]) -> Result<(), Error> {
        assert!(iv.len() == 16);
        let result = unsafe { SaSi_AesGetIv(self.context(), iv.as_mut_ptr()) };
        if result != 0 {
            return Err(Error::Other(result));
        }
        Ok(())
    }

    /// Process a block of data
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

    /// Finish the cipher operation
    fn finish(&mut self, input: &[u8], output: &mut [u8]) -> Result<(), Error> {
        assert!(input.len() <= output.len());
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

const AAD_B0_LEN: usize = BLOCK_SIZE - LENGTH_FIELD_LENGTH;

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

    /// Create
    fn make_flag(a_length: usize, big_m: usize, big_l: usize) -> u8 {
        let mut flag = if a_length > 0 { 0x40 } else { 0 };
        flag = if big_m > 0 {
            flag | ((((big_m - 2) / 2) as u8) & 0x07) << 3
        } else {
            flag
        };
        flag |= 0x07 & ((big_l - 1) as u8);
        flag
    }
}

impl CryptoBackend for CryptoCellBackend {
    fn ccmstar_decrypt(
        &mut self,
        key: &[u8],
        nonce: &[u8],
        encrypted: &[u8],
        mic: &[u8],
        aad: &[u8],
        decrypted: &mut [u8],
    ) -> Result<usize, Error> {
        assert!(key.len() == KEY_SIZE);
        assert!(nonce.len() == 13);
        assert!(decrypted.len() >= encrypted.len());

        let enc_full_block_length = (encrypted.len() / BLOCK_SIZE) * BLOCK_SIZE;

        let mut tag = [0; BLOCK_SIZE];
        {
            // Decrypt data
            let mut cipher = AesContext::new(
                EncryptDecrypt::Decrypt,
                AesOperationMode::Ctr,
                PaddingType::None,
            );
            cipher.set_key(key)?;

            let mut block = [0u8; BLOCK_SIZE];
            {
                let (flag, other) = block.split_at_mut(1);
                let (_nonce, _counter) = other.split_at_mut(nonce.len());
                flag[0] = Self::make_flag(0, 0, LENGTH_FIELD_LENGTH);
                _nonce.copy_from_slice(&nonce);
            }

            cipher.set_iv(&block)?;

            let mut block = [0u8; BLOCK_SIZE];
            block[..mic.len()].copy_from_slice(&mic);

            cipher.process_block(&block, &mut tag)?;

            if enc_full_block_length > 0 {
                cipher.process_block(&encrypted[..enc_full_block_length], decrypted)?;
            }

            cipher.finish(
                &encrypted[enc_full_block_length..],
                &mut decrypted[enc_full_block_length..],
            )?;
        }
        let mut output = [0u8; BLOCK_SIZE];
        {
            // Validate MIC using AES128-CBC-MAC
            let mut cipher = AesContext::new(
                EncryptDecrypt::Encrypt,
                AesOperationMode::CbcMac,
                PaddingType::None,
            );
            cipher.set_key(key)?;

            let length_field = encrypted.len() as u16;

            let mut block = [0u8; BLOCK_SIZE];
            {
                let (flag, other) = block.split_at_mut(1);
                let (_nonce, length) = other.split_at_mut(nonce.len());
                flag[0] = Self::make_flag(aad.len(), mic.len(), LENGTH_FIELD_LENGTH);
                _nonce.copy_from_slice(&nonce);
                length[0] = (length_field >> 8) as u8;
                length[1] = (length_field & 0x00ff) as u8;
            }

            cipher.process_block(&block, &mut output)?;

            // Feed the additional data
            let mut block = [0u8; BLOCK_SIZE];
            let aad_length = aad.len() as u16;
            block[0] = (aad_length >> 8) as u8;
            block[1] = (aad_length & 0x00ff) as u8;
            let len = if aad.len() < AAD_B0_LEN {
                aad.len()
            } else {
                AAD_B0_LEN
            };
            block[2..2 + len].copy_from_slice(&aad[..len]);

            cipher.process_block(&block, &mut output)?;

            if aad.len() > AAD_B0_LEN {
                let mut iter = aad[AAD_B0_LEN..].chunks_exact(BLOCK_SIZE);
                loop {
                    match iter.next() {
                        Some(input) => {
                            cipher.process_block(input, &mut output)?;
                        }
                        None => {
                            let input = iter.remainder();
                            if !input.is_empty() {
                                let mut block = [0u8; BLOCK_SIZE];
                                block[..input.len()].copy_from_slice(input);
                                cipher.process_block(&block, &mut output)?;
                            }
                            break;
                        }
                    }
                }
            }

            let block_last = ((encrypted.len() + (BLOCK_SIZE - 1)) / BLOCK_SIZE) - 1;
            let mut block_index = 0;
            let mut iter = decrypted[..encrypted.len()].chunks_exact(BLOCK_SIZE);
            loop {
                match iter.next() {
                    Some(input) => {
                        if block_index < block_last {
                            cipher.process_block(input, &mut output)?;
                        }
                        else {
                            cipher.finish(input, &mut output)?;
                        }
                    }
                    None => {
                        let mut block = [0u8; BLOCK_SIZE];
                        let input = iter.remainder();
                        block[..input.len()].copy_from_slice(input);
                        // Must feed a full block into finish here, otherwise the
                        // result will be wrong
                        cipher.finish(&block, &mut output)?;
                        break;
                    }
                }
                block_index += 1;
            }
        }

        let mut valid = true;
        for (a, b) in tag[..mic.len()].iter().zip(output[..mic.len()].iter()) {
            if a != b {
                valid = false;
                break;
            }
        }

        if valid {
            Ok(encrypted.len())
        } else {
            clear(decrypted);
            Ok(0)
        }
    }

    fn ccmstar_encrypt(
        &mut self,
        key: &[u8],
        nonce: &[u8],
        message: &[u8],
        mic: &mut [u8],
        aad: &[u8],
        output: &mut [u8],
    ) -> Result<usize, psila_crypto::Error> {
        let mut new_mic = [0u8; BLOCK_SIZE];
        // Generate a MIC
        {
            let aad_blocks = (aad.len() + LENGTH_FIELD_LENGTH + (BLOCK_SIZE - 1)) / BLOCK_SIZE;
            let mut buffer = [0u8; 256];
            let mut offset = 0;

            buffer[0] = Self::make_flag(aad.len(), mic.len(), LENGTH_FIELD_LENGTH);
            offset += 1;
            buffer[offset..offset + nonce.len()].copy_from_slice(nonce);
            offset += nonce.len();
            let message_len = message.len() as u16;
            buffer[offset] = (message_len >> 8) as u8;
            buffer[offset + 1] = (message_len & 0x00ff) as u8;
            offset += 2;
            let aad_len = aad.len() as u16;
            buffer[offset] = (aad_len >> 8) as u8;
            buffer[offset + 1] = (aad_len & 0x00ff) as u8;
            offset += 2;
            buffer[offset..offset + aad.len()].copy_from_slice(aad);
            offset += (aad_blocks * BLOCK_SIZE) - 2;
            buffer[offset..offset + message.len()].copy_from_slice(message);
            offset += message.len();

            let mut cipher = AesContext::new(
                EncryptDecrypt::Encrypt,
                AesOperationMode::CbcMac,
                PaddingType::None,
            );
            cipher.set_key(key)?;

            let mut iter = buffer[..offset].chunks_exact(BLOCK_SIZE);
            loop {
                match iter.next() {
                    Some(input) => {
                        cipher.process_block(&input, &mut new_mic)?;
                    }
                    None => {
                        let mut block = [0u8; BLOCK_SIZE];
                        block[..iter.remainder().len()].copy_from_slice(iter.remainder());
                        cipher.finish(&block, &mut new_mic)?;
                        break;
                    }
                }
            }
        }
        {
            let message_blocks = (message.len() + (BLOCK_SIZE - 1)) / BLOCK_SIZE;
            let mut buffer = [0u8; 256];
            let mut encrypted = [0u8; 256];
            let mut offset = 0;

            buffer[..message.len()].copy_from_slice(message);
            offset += message_blocks * BLOCK_SIZE;

            let mut block = [0u8; BLOCK_SIZE];
            block[0] = Self::make_flag(0, 0, LENGTH_FIELD_LENGTH);
            block[1..=nonce.len()].copy_from_slice(nonce);

            let mut cipher = AesContext::new(
                EncryptDecrypt::Encrypt,
                AesOperationMode::Ctr,
                PaddingType::None,
            );
            cipher.set_key(key)?;
            cipher.set_iv(&block)?;

            let mut block = [0u8; BLOCK_SIZE];
            let mut tag = [0u8; 16];
            block[..mic.len()].copy_from_slice(&new_mic[..mic.len()]);
            cipher.process_block(&block, &mut tag)?;

            for (o, i) in encrypted[..offset]
                .chunks_mut(BLOCK_SIZE)
                .zip(buffer.chunks(BLOCK_SIZE))
            {
                cipher.process_block(i, o)?;
            }

            output[..message.len()].copy_from_slice(&encrypted[..message.len()]);
            mic.copy_from_slice(&tag[..mic.len()]);
        }

        Ok(message.len())
    }

    /// Set the key
    fn aes128_ecb_encrypt_set_key(&mut self, key: &[u8]) -> Result<(), Error> {
        self.cipher.set_key(key)
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
