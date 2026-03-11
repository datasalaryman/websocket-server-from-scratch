use nix::{NixPath, sys::socket::{
    AddressFamily, Backlog, MsgFlags, SockFlag, SockType, SockaddrIn, accept, bind, listen, recv,
    send, setsockopt, socket, sockopt::ReuseAddr,
}};
use std::{os::fd::RawFd, str::FromStr};

#[derive(Debug)]
pub enum ExtendedPayloadLen {
    Zero(u8), // when 0
    One(u16),
    Two(u64),
}

#[derive(Debug)]
pub struct WrongLengthSpeficied; 

impl std::fmt::Display for WrongLengthSpeficied {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Invalid expected payload length specified, can only accept 0, 16, or 64 bits")
    }
}


impl TryFrom<Vec<u8>> for ExtendedPayloadLen {
    type Error = WrongLengthSpeficied;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {

        return match value.len() {
            0 => Ok(Self::Zero(0 as u8)), 
            16 => {
                let bytes:  [u8; 2] = value.as_slice().try_into().unwrap();
                Ok(Self::One(u16::from_le_bytes(bytes)))
            }, 
            64 => {
                let bytes:  [u8; 8] = value.as_slice().try_into().unwrap();
                Ok(Self::Two(u64::from_le_bytes(bytes)))
            }, 
            _ => Err(WrongLengthSpeficied),
        };
    }
}

impl From<ExtendedPayloadLen> for usize {
    fn from (id: ExtendedPayloadLen) -> usize {
        match id {
            ExtendedPayloadLen::Zero(v) => v as usize, 
            ExtendedPayloadLen::One(v) => v as usize, 
            ExtendedPayloadLen::Two(v) => v as usize
        } 
    }
}

#[derive(Debug)]
pub struct Frame {
    pub fin: bool,
    pub opcode: u8,
    pub masked: bool,
    pub masking_key: String,
    pub payload_data: String,
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
        let payload_len = (header_buf[1] & 0x7F) as u64;

        let mut extended_payload_len_buffer: Vec<u8> = vec![];

        if payload_len == 127 {
            total_read = 0;
            while total_read < 2 {
                let n = recv(
                    fd,
                    &mut extended_payload_len_buffer[total_read..],
                    MsgFlags::empty(),
                )
                .unwrap();

                if n == 0 {
                    break;
                }

                total_read += n;
            }

        
        } else if payload_len == 128 {
            total_read = 0;
            while total_read < 8 {
                let n = recv(
                    fd,
                    &mut extended_payload_len_buffer[total_read..],
                    MsgFlags::empty(),
                )
                .unwrap();

                if n == 0 {
                    break;
                }

                total_read += n;
            }

        }

        let extended_payload_len = ExtendedPayloadLen::try_from(extended_payload_len_buffer).unwrap();

        let mut masking_key_buffer = [0u8; 4]; 

        if masked {
            total_read = 0;
            while total_read < 4 {
                let n = recv(
                    fd, 
                    &mut masking_key_buffer[total_read..], 
                    MsgFlags::empty()
                )
                .unwrap(); 

                if n == 0 {
                    break;
                }
                
                total_read += n; 
            }
        }

        let masking_key = String::from_utf8_lossy(&masking_key_buffer).to_string();

        let mut payload_buffer : Vec<u8> = vec![];

        if [126u64, 127u64].contains(&payload_len) {
            total_read = 0; 
            let target_length = extended_payload_len.into(); 
            while total_read < target_length {
                let n = recv(
                    fd, 
                    &mut payload_buffer[total_read..], 
                    MsgFlags::empty()
                ).unwrap();

                if n == 0 {
                    break;
                }

                total_read += n;
            }
        } else {
            total_read = 0;
            while total_read < payload_len as usize {  
                let n = recv(
                    fd, 
                    &mut payload_buffer[total_read..], 
                    MsgFlags::empty()
                ).unwrap();

                if n == 0 {
                    break; 
                }
                
                total_read += n;
            }
        }

        let payload_data = String::from_utf8_lossy(&payload_buffer).to_string();

        Ok(Self {
            fin,
            opcode,
            masked,
            masking_key, 
            payload_data,
        })
    }
}

#[derive(Debug)]
pub struct Message {
    pub frames: Vec<Frame>, 
    pub message: String, 
}

impl Message {
    pub fn from(fd: RawFd) -> Message {
        let mut frames : Vec<Frame> = vec![];
        let mut message: String = String::new();
        loop {
            let frame = Frame::try_from(fd).unwrap();
            
            if frame.fin {
                frames.push(frame); 
                message = frames.iter().map(|x| x.payload_data.clone()).collect::<Vec<String>>().join("");
                break;

            } else {
                frames.push(frame); 
            }
        }

        let message_struct = Message {
            frames, 
            message
        }; 

        return message_struct;
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", 
            self.message
        )
    }
}


