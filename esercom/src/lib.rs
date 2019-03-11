#![no_std]

pub mod slip;
pub mod error;

#[derive(Debug, PartialEq)]
pub enum MessageType {
    None = 0,
    Error = 1,
    GetValue = 2,
    SetValue = 3,
    RadioReceive = 4,
    RadioSend = 5,
}

impl MessageType {
     fn from_bits(bits: u8) -> Self {
     	match bits {
	      1 => MessageType::Error,
      	      2 => MessageType::GetValue,
      	      3 => MessageType::SetValue,
      	      4 => MessageType::RadioReceive,
      	      5 => MessageType::RadioSend,
	      _ => MessageType::None,
	}
     }
}

pub fn com_encode(mt: MessageType, input: &[u8], output: &mut [u8]) -> Result<usize, error::Error>
{
    if input.len() > 128 {
        return Err(error::Error::NotEnoughBytes);
    }
    let mut buf = [0u8; 256];
    let length = (input.len() as u8) + 2;
    buf[0] = length;
    buf[1] = mt as u8;
    let length = length as usize;
    buf[2..length].copy_from_slice(input);
    let (_, out_size) = slip::encode(&buf[..length], output)?;
    Ok(out_size)
}

pub fn com_decode(input: &[u8], output: &mut [u8]) -> Result<(MessageType, usize, usize), error::Error>
{
    let mut buf = [0u8; 256];
    let (in_size, out_size) = slip::decode(input, &mut buf)?;
    if out_size < 2 || out_size != (buf[0] as usize) {
        return Err(error::Error::InvalidLength(in_size));
    }
    let mt = MessageType::from_bits(buf[1]);
    let length = out_size - 2;
    output[..length].copy_from_slice(&buf[2..out_size]);
    Ok((mt, in_size, length))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        let data = [0x01, 0x8e];
        let mut buf = [0u8; 32];
        let size = com_encode(MessageType::GetValue, &data, &mut buf).unwrap();
        assert_eq!(size, 6);
        assert_eq!(buf[..size], [0xc0, 0x04, 0x02, 0x01, 0x8e, 0xc0]);
    }

    #[test]
    fn decode() {
        let mut buf = [0u8; 32];
        let data = [0xc0, 0x04, 0x02, 0x01, 0x8e, 0xc0];
        let (mt, used, size) = com_decode(&data, &mut buf).unwrap();
        assert_eq!(used, 6);
        assert_eq!(size, 2);
        assert_eq!(mt, MessageType::GetValue);

        let data = [0x04, 0x02, 0x01, 0x8e, 0xc0];
        let (mt, used, size) = com_decode(&data, &mut buf).unwrap();
        assert_eq!(used, 5);
        assert_eq!(size, 2);
        assert_eq!(mt, MessageType::GetValue);

        let data = [0xfe, 0x01, 0x8e, 0xc0];
        let result = com_decode(&data, &mut buf);
        assert_eq!(result, Err(error::Error::InvalidLength(4)));
    }
}
