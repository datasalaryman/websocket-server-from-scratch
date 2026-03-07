use sha1::{Sha1, Digest};
use crate::request::{Request}; 
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Debug)]
pub struct Header{
    pub key: String, 
    pub value: String 
}

#[derive(Debug)]
pub struct Response<'a> {
    pub version: &'a str, 
    pub status: &'a u8,
    pub headers: Vec<Header>, 
}

#[derive(Debug)]
pub struct NotFoundError;

impl std::fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nConnection: close")
    }
}

impl std::error::Error for NotFoundError {}

#[derive(Debug)]
pub struct InternalServerError;

impl std::fmt::Display for InternalServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
         write!(f, "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nAllow: GET\r\nConnection: close")
    }
}

impl std::error::Error for InternalServerError {}

pub enum ResponseError {
    NotFoundError, 
    InternalServerError
}

impl<'a> TryFrom<&Request<'a>> for Response<'a> {

    type Error = ResponseError; 

    fn try_from(s: &Request<'a>) -> Result<Self, Self::Error> {
        if s.route != "/" {
            return Err(ResponseError::NotFoundError)
        }

        let client_key = s.headers.iter().filter(|x| x.is_key("Sec-WebSocket-Key")).collect::<Vec<_>>()[0].value.to_string() + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

        let key_to_sha1 = Sha1::digest(client_key.as_bytes());
        let sha1_to_base64 = STANDARD.encode(&key_to_sha1);

        let headers = vec![
            Header {
                key: "Upgrade".to_string(), 
                value: "websocket".to_string()
            }, 
            Header {
                key: "Connection".to_string(), 
                value: "Upgrade".to_string()
            }, 
            Header {
                key: "Sec-WebSocket-Accept".to_string(), 
                value: sha1_to_base64
            }
        ];

        Ok(Self {
            version: s.version, 
            status: &101, 
            headers, 
        })
    }
}

impl<'a> std::fmt::Display for Response<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {

        let headers = self.headers
            .iter()
            .map(|x| format!(
                "{}: {}\r\n", 
                x.key, 
                x.value
            ))
            .collect::<Vec<_>>()
            .join(""); 

        write!(f, "{} {} Switching Protocols\r\n{}\r\n", 
            self.version, 
            self.status,
            headers.as_str(), 
        )
    }
}
