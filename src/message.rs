use nix::{
    sys::socket::{
        MsgFlags, 
        recv, 
    },
};
use std::{os::fd::RawFd};

use crate::{BadRequestError, ConnMap};

#[derive(Debug, Clone)]
pub struct Frame {
    pub fin: bool,
    pub opcode: u8,
    pub masked: bool,
    pub masking_key: [u8; 4],
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub struct FrameError {}

impl Frame {
    pub fn try_from(fd: RawFd) -> Result<Frame, FrameError> {
        type Error = FrameError;

        // This reads the first two bytes of the frame. They contain:
        // FIN - 1 bit
        // RSV1-3 - 3 bits,
        // Opcode - 4 bits,
        // Mask - 1 bit
        // Payload length - 7 bits
        let mut header_buf = [0u8; 2];

        let mut total_read = 0;
        while total_read < 2 {
            let n = recv(fd, &mut header_buf[total_read..], MsgFlags::empty()).unwrap();
            if n == 0 {
                break; // Connection closed
            }
            total_read += n;
        }

        let fin = (header_buf[0] & 0x80) != 0;
        let opcode = header_buf[0] & 0x0F;

        let masked = (header_buf[0] & 0x80) != 0;
        let mut payload_len = (header_buf[1] & 0x7F) as u64;

        let mut extended_len_bytes = 0;
        if payload_len == 126 {
            extended_len_bytes = 2;
        } else if payload_len == 127 {
            extended_len_bytes = 8;
        }

        // Read extended length if needed
        let mut extended_buf = [0u8; 8];
        if extended_len_bytes > 0 {
            let mut read = 0;
            while read < extended_len_bytes {
                let n = recv(
                    fd,
                    &mut extended_buf[read..extended_len_bytes],
                    MsgFlags::empty(),
                )
                .unwrap();
                if n == 0 {
                    break;
                }
                read += n;
            }

            payload_len = match extended_len_bytes {
                2 => u16::from_be_bytes([extended_buf[0], extended_buf[1]]) as u64,
                8 => u64::from_be_bytes(extended_buf),
                _ => payload_len,
            };
        }

        let mut masking_key = [0u8; 4];

        if masked {
            total_read = 0;
            while total_read < 4 {
                let n = recv(fd, &mut masking_key[total_read..], MsgFlags::empty()).unwrap();

                if n == 0 {
                    break;
                }

                total_read += n;
            }
        }

        let mut payload = vec![0u8; payload_len as usize];

        let mut total_read = 0;
        while total_read < payload.len() {
            let n = recv(fd, &mut payload[total_read..], MsgFlags::empty()).unwrap();

            if n == 0 {
                break;
            }

            total_read += n;
        }

        if masked {
            for i in 0..payload.len() {
                payload[i] ^= masking_key[i % 4];
            }
        }

        Ok(Self {
            fin,
            opcode,
            masked,
            masking_key,
            payload,
        })
    }

    pub fn get_close_frame(close: &ClientMessage) -> Frame {

        let close_frame = close.frames[0].clone(); 

        let frame = Frame {
            fin: true,
            opcode: 8,
            masked: false,
            masking_key: [0u8; 4],
            payload: close_frame.payload,
        };

        return frame;
    }

    pub fn get_pong_frame(ping: &ClientMessage) -> Frame {

        let ping_frame = ping.frames[0].clone();


        let frame = Frame {
            fin: true,
            opcode: 0x0A,
            masked: false,
            masking_key: [0u8; 4],
            payload: ping_frame.payload,
        };

        return frame;
    }

    pub fn as_bytes(f: Frame) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        let fin_opcode: u8 = ((f.fin as u8) << 7) | (f.opcode as u8);

        bytes.push(fin_opcode);

        let mut mask_payload_len: u8;

        println!("payload_len: {}", f.payload.len());
        
        if f.payload.len() <= 125 {

            mask_payload_len = ((0 as u8) << 7) | (f.payload.len() as u8);
            bytes.push(mask_payload_len);

        } else if ( f.payload.len() as u16 ) < 65_535 {

            mask_payload_len = ((0 as u8) << 7) | (126 as u8);
            bytes.push(mask_payload_len);

            let extended_len : [u8; 2] = (f.payload.len() as u16).to_be_bytes();
            
            bytes.extend(extended_len);

        } else { 
            
            mask_payload_len = ((0 as u8) << 7) | (127 as u8);
            bytes.push(mask_payload_len);

            let extended_len : [u8; 8] = (f.payload.len() as u64).to_be_bytes();
            
            bytes.extend(extended_len);


        }

        let payload = f.payload.clone();

        bytes.extend(payload);

        return bytes;
    }

    pub fn unmask(f: Frame) -> Frame {
        let unmasked = Frame {
            masked: false, 
            ..f
        };

        return unmasked;
    }
}

#[derive(Debug, Clone)]
pub struct ClientMessage {
    pub frames: Vec<Frame>,
    pub opcode: u8,
    pub message: String,
}

impl ClientMessage {
    pub fn from(fd: RawFd) -> Result<ClientMessage, FrameError> {
        type Error = FrameError;
        let mut frames: Vec<Frame> = vec![];
        let mut message: String = String::new();
        loop {
            let frame = Frame::try_from(fd).unwrap();

            if frame.fin || frame.opcode == 0x8 {
                frames.push(frame);
                message = frames
                    .iter()
                    .map(|x| String::from_utf8_lossy(&x.payload.clone()).to_string())
                    .collect::<Vec<String>>()
                    .join("");
                break;
            } else {
                frames.push(frame);
            }
        }

        let opcode = frames[0].opcode;

        let message_struct = ClientMessage {
            frames,
            opcode,
            message,
        };

        return Ok(message_struct);
    }
}

impl std::fmt::Display for ClientMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Debug)]
pub struct OpcodeNotRecognizedError;

impl std::fmt::Display for OpcodeNotRecognizedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Invalid opcode specified, not recognized")
    }
}

#[derive(Debug)]
pub struct ServerMessage {
    pub frames: Vec<Frame>,
    pub opcode: u8,
    pub message: String,
}

impl ServerMessage {
    pub fn from(cmsg: &ClientMessage) -> Result<Self, OpcodeNotRecognizedError> {
        type Error = OpcodeNotRecognizedError;
        match cmsg.opcode {
            // if text (1) or binary (2), send the message back to the client,
            1 | 2 => {
                return Ok(Self {
                    frames: cmsg.frames
                        .iter()
                        .map(|x| Frame::unmask(x.clone()))
                        .collect(),
                    opcode: cmsg.opcode,
                    message: cmsg.message.clone(),
                });
            }
            // if close connection (8), send back close frame
            8 => {
                return Ok(Self {
                    frames: vec![(Frame::get_close_frame(&cmsg))],
                    opcode: cmsg.opcode,
                    message: cmsg.message.clone(),
                });
            }

            // if ping (9), send pong
            9 => {
                return Ok(Self {
                    frames: vec![(Frame::get_pong_frame(&cmsg))],
                    opcode: 0x0A,
                    message: cmsg.message.clone(),
                });
            }

            _ => return Err(OpcodeNotRecognizedError),
        };
    }
}

impl std::fmt::Display for ServerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
