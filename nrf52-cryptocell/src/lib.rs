//! nRF52840 CryptoCell

#![no_std]

use nrf52840_pac::CRYPTOCELL;

mod nrf_cc310;

/// L, length of the message length field in octets 2, 3, ... 8. Always 2 for Zigbee
pub const LENGHT_FIELD_LENGTH: usize = 2;
/// Cipher block length
pub const BLOCK_SIZE: usize = 16;
/// Key length
pub const KEY_SIZE: usize = 16;

pub enum Error {
    CryptoCellError(u32),
}

extern "C" {
    pub fn CC_AESCCM(
        EncrDecrMode: nrf_cc310::SaSiAesEncryptMode_t,
        CCM_Key: *const u8,
        KeySizeId: nrf_cc310::CRYS_AESCCM_KeySize_t,
        N_ptr: *const u8,
        SizeOfN: u8,
        ADataIn_ptr: *const u8,
        ADataInSize: u32,
        TextDataIn_ptr: *const u8,
        TextDataInSize: u32,
        TextDataOut_ptr: *mut u8,
        SizeOfT: u8,
        Mac_Res: *mut u8,
        ccmMode: u32,
    ) -> nrf_cc310::CRYSError_t;
}

pub struct CryptoCell {
    cc: CRYPTOCELL,
}

impl core::ops::Drop for CryptoCell {
    fn drop(&mut self) {
        unsafe { nrf_cc310::SaSi_LibFini() }
        self.cc.enable.write(|w| w.enable().clear_bit());
    }
}

impl CryptoCell {
    pub fn new(cc: CRYPTOCELL) -> Self {
        cc.enable.write(|w| w.enable().set_bit());

        if unsafe { nrf_cc310::SaSi_LibInit() } != nrf_cc310::SA_SilibRetCode_t_SA_SILIB_RET_OK {
            panic!("Failed to initialize SaSi library");
        }

        Self { cc }
    }

    pub fn aes128_ccm_star_decrypt(
        &self,
        key: &[u8; KEY_SIZE],
        nonce: &[u8],
        message: &[u8],
        mic_length: usize,
        additional_data: &[u8],
        message_output: &mut [u8],
    ) -> Result<(), Error> {
        let message_len = (message.len() - mic_length) as u32;
        let mut mic_result = [0u8; 16];
        mic_result[..mic_length].copy_from_slice(&message[message_len as usize..]);
        let result = unsafe {
            CC_AESCCM(
                nrf_cc310::SaSiAesEncryptMode_t_SASI_AES_DECRYPT,
                key.as_ptr(),
                nrf_cc310::CRYS_AESCCM_KeySize_t_CRYS_AES_Key128BitSize,
                nonce.as_ptr(),
                nonce.len() as u8,
                additional_data.as_ptr(),
                additional_data.len() as u32,
                message.as_ptr(),
                message_len,
                message_output.as_mut_ptr(),
                mic_length as u8,
                mic_result.as_mut_ptr(),
                nrf_cc310::CRYS_AESCCM_MODE_STAR,
            )
        };
        if result != nrf_cc310::CRYS_OK {
            return Err(Error::CryptoCellError(result));
        }
        Ok(())
    }
}
