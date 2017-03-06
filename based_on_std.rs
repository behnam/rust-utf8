use std::cmp;
use std::str;

include!("polyfill.rs");

#[derive(Debug, Copy, Clone)]
pub enum DecodeResult<'a> {
    Ok(&'a str),
    Error(&'a str, InvalidSequence<'a>, &'a [u8]),
    Incomplete(&'a str, IncompleteChar),
}

#[derive(Debug, Copy, Clone)]
pub struct InvalidSequence<'a>(pub &'a [u8]);

#[derive(Debug, Copy, Clone)]
pub struct IncompleteChar {
    buffer: [u8; 4],
    buffer_len: u8,
}

pub fn decode(input: &[u8]) -> DecodeResult {
    let error = match str::from_utf8(input) {
        Ok(valid) => return DecodeResult::Ok(valid),
        Err(error) => error,
    };

    // FIXME: separate function from here to guide inlining?
    let valid_up_to = error.valid_up_to();
    let (valid, after_valid) = input.split_at(valid_up_to);
    let valid = unsafe {
        str::from_utf8_unchecked(valid)
    };

    match utf8error_resume_from(&error, input) {
        Some(resume_from) => {
            let invalid_sequence_length = resume_from - valid_up_to;
            let (invalid, rest) = after_valid.split_at(invalid_sequence_length);
            DecodeResult::Error(valid, InvalidSequence(invalid), rest)
        }
        None => {
            let mut buffer = [0, 0, 0, 0];
            let after_valid = &input[error.valid_up_to()..];
            buffer[..after_valid.len()].copy_from_slice(after_valid);
            DecodeResult::Incomplete(valid, IncompleteChar {
                buffer: buffer,
                buffer_len: after_valid.len() as u8,
            })
        }
    }
}

impl IncompleteChar {
    /// * `None`: still incomplete, call `try_complete` again with more input.
    /// * `Some((result, rest))`: We’re done with this `IncompleteChar`.
    ///   To keep decoding, pass `rest` to `decode()`.
    pub fn try_complete<'char, 'input>(&'char mut self, input: &'input [u8])
                                       -> Option<(Result<&'char str, &'char [u8]>, &'input [u8])> {
        let buffer_len = self.buffer_len as usize;
        let bytes_from_input;
        {
            let unwritten = &mut self.buffer[buffer_len..];
            bytes_from_input = cmp::min(unwritten.len(), input.len());
            unwritten[..bytes_from_input].copy_from_slice(&input[..bytes_from_input]);
        }
        let spliced = &self.buffer[..buffer_len + bytes_from_input];
        match str::from_utf8(spliced) {
            Ok(valid) => {
                Some((Ok(valid), &input[bytes_from_input..]))
            }
            Err(error) => {
                let valid_up_to = error.valid_up_to();
                if valid_up_to > 0 {
                    let valid = &self.buffer[..valid_up_to];
                    let valid = unsafe {
                        str::from_utf8_unchecked(valid)
                    };
                    assert!(valid_up_to > buffer_len);
                    let bytes_from_input = valid_up_to - buffer_len;
                    Some((Ok(valid), &input[bytes_from_input..]))
                } else {
                    match utf8error_resume_from(&error, spliced) {
                        Some(resume_from) => {
                            let invalid = &spliced[..resume_from];
                            assert!(resume_from > buffer_len);
                            let bytes_from_input = resume_from - buffer_len;
                            let rest = &input[bytes_from_input..];
                            Some((Err(invalid), rest))
                        }
                        None => None
                    }
                }
            }
        }
    }
}
