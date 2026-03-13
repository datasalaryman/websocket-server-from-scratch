use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub struct RequestHeader<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

impl PartialEq for RequestHeader<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && self.value == other.value
    }
}

impl RequestHeader<'_> {
    pub fn is_key(&self, key: &str) -> bool {
        self.key == key
    }
}

#[derive(Debug)]
pub struct Request<'a> {
    pub method: &'a str,
    pub route: &'a str,
    pub version: &'a str,
    pub headers: Vec<RequestHeader<'a>>,
}

#[derive(Debug)]
pub struct BadRequestError;

impl std::fmt::Display for BadRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nConnection: close"
        )
    }
}

impl std::error::Error for BadRequestError {}

#[derive(Debug)]
pub struct MethodNotAllowedError;

impl std::fmt::Display for MethodNotAllowedError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/plain\r\nAllow: GET"
        )
    }
}

impl std::error::Error for MethodNotAllowedError {}

pub enum RequestError {
    BadRequestError,
    MethodNotAllowedError,
}

impl std::fmt::Display for RequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let error_message = match self {
            RequestError::BadRequestError => BadRequestError.to_string(),
            RequestError::MethodNotAllowedError => MethodNotAllowedError.to_string(),
        };

        write!(f, "{}", error_message)
    }
}

impl<'a> TryFrom<&'a str> for Request<'a> {
    type Error = RequestError;

    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        let request_lines: Vec<&str> = s.split("\r\n").collect();

        let start_line: Vec<&str> = request_lines[0].split(" ").collect();
        if start_line.len() < 2 {
            return Err(RequestError::BadRequestError);
        }
        // Only allow GET methods for now
        if start_line[0] != "GET" {
            return Err(RequestError::MethodNotAllowedError);
        }
        if start_line[2] != "HTTP/1.1" {
            return Err(RequestError::BadRequestError);
        }

        let mut headers_lines: Vec<&str> = Vec::new();
        for (i, line) in request_lines.iter().enumerate() {
            if line.is_empty() {
                headers_lines = request_lines[1..i].to_vec();
                break;
            }
        }

        let mut headers = Vec::new();
        for header_line in &headers_lines {
            let (k, v) = header_line.split_once(": ").unwrap();
            headers.push(RequestHeader { key: k, value: v });
        }

        let required_websocket_headers = vec![
            RequestHeader {
                key: "Upgrade",
                value: "websocket",
            },
            RequestHeader {
                key: "Connection",
                value: "Upgrade",
            },
            RequestHeader {
                key: "Sec-WebSocket-Version",
                value: "13",
            },
        ];

        let has_all_required = required_websocket_headers
            .iter()
            .all(|x| headers.contains(x));
        let has_websocket_key = headers.iter().any(|b| b.key == "Sec-WebSocket-Key");

        if !has_all_required || !has_websocket_key {
            return Err(RequestError::BadRequestError);
        }

        Ok(Request {
            method: &start_line[0],
            route: &start_line[1],
            version: &start_line[2],
            headers,
        })
    }
}

#[derive(Debug)]
pub struct Header {
    pub key: String,
    pub value: String,
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
        write!(
            f,
            "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nConnection: close"
        )
    }
}

impl std::error::Error for NotFoundError {}

#[derive(Debug)]
pub struct InternalServerError;

impl std::fmt::Display for InternalServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nAllow: GET\r\nConnection: close"
        )
    }
}

impl std::error::Error for InternalServerError {}

pub enum ResponseError {
    NotFoundError,
    InternalServerError,
}

impl<'a> TryFrom<&Request<'a>> for Response<'a> {
    type Error = ResponseError;

    fn try_from(s: &Request<'a>) -> Result<Self, Self::Error> {
        if s.route != "/" {
            return Err(ResponseError::NotFoundError);
        }

        let client_key = s
            .headers
            .iter()
            .filter(|x| x.is_key("Sec-WebSocket-Key"))
            .collect::<Vec<_>>()[0]
            .value
            .to_string()
            + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

        let key_to_sha1 = Sha1::digest(client_key.as_bytes());
        let sha1_to_base64 = STANDARD.encode(&key_to_sha1);

        let headers = vec![
            Header {
                key: "Upgrade".to_string(),
                value: "websocket".to_string(),
            },
            Header {
                key: "Connection".to_string(),
                value: "Upgrade".to_string(),
            },
            Header {
                key: "Sec-WebSocket-Accept".to_string(),
                value: sha1_to_base64,
            },
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
        let headers = self
            .headers
            .iter()
            .map(|x| format!("{}: {}\r\n", x.key, x.value))
            .collect::<Vec<_>>()
            .join("");

        write!(
            f,
            "{} {} Switching Protocols\r\n{}\r\n",
            self.version,
            self.status,
            headers.as_str(),
        )
    }
}
