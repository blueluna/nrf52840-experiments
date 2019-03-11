use crate::error::Error;

/// Frame end
const END: u8 = 0xC0;

/// Frame escape
const ESC: u8 = 0xDB;

/// Escaped end character
const ESC_END: u8 = 0xDC;

/// Escaped escape character
const ESC_ESC: u8 = 0xDD;

fn calc_size(input: &[u8]) -> usize {
    if input.len() == 0 {
        return 0;
    }
    let mut size = 2; // END + END
    for n in 0..input.len() {
        match input[n] {
            ESC | END => {
                size += 2;
            }
            _ => {
                size += 1;
            }
        }
    }
    size
}

pub fn encode(input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
    let out_size = calc_size(input);
    if out_size == 0 {
        return Ok((0, 0));
    }
    if output.len() < out_size {
        return Err(Error::NotEnoughBytes);
    }
    let mut in_idx = 0;
    output[0] = END;
    let mut out_idx = 1;
    loop {
        match input[in_idx] {
            ESC => {
                output[out_idx] = ESC;
                output[out_idx + 1] = ESC_ESC;
                out_idx += 2;
            }
            END => {
                output[out_idx] = ESC;
                output[out_idx + 1] = ESC_END;
                out_idx += 2;
            }
            _ => {
                output[out_idx] = input[in_idx];
                out_idx += 1;
            }
        }
        in_idx += 1;
        if in_idx >= input.len() {
            break;
        }
    }
    output[out_idx] = END;
    out_idx += 1;
    Ok((in_idx, out_idx))
}

pub fn decode(input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
    if input.is_empty() {
        return Ok((0, 0));
    }
    if output.len() < input.len() {
        return Err(Error::NotEnoughBytes);
    }
    let mut in_idx = 0;
    let mut out_idx = 0;
    loop {
        match input[in_idx] {
            ESC => {
                if (in_idx + 1) < input.len() {
                    match input[in_idx + 1] {
                        ESC_ESC => {
                            output[out_idx] = ESC;
                            out_idx += 1;
                        }
                        ESC_END => {
                            output[out_idx] = END;
                            out_idx += 1;
                        }
                        _ => {
                            return Err(Error::InvalidEscapeSequence);
                        }
                    }
                    in_idx += 1;
                }
            }
            END => {
                if in_idx > 0 {
                    break;
                }
            }
            _ => {
                output[out_idx] = input[in_idx];
                out_idx += 1;
            }
        }
        in_idx += 1;
        if in_idx >= input.len() {
            return Err(Error::EndNotFound);
        }
    }
    Ok((in_idx + 1, out_idx))
}
