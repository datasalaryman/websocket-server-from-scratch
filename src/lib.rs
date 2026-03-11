use nix::sys::socket::{
    AddressFamily, Backlog, MsgFlags, SockFlag, SockType, SockaddrIn, accept, bind, listen, recv,
    send, setsockopt, socket, sockopt::ReuseAddr,
};
use nix::unistd::close;
use std::os::fd::AsRawFd;
use std::{os::fd::RawFd, str::FromStr};

pub mod message;
pub mod request;
pub mod response;
pub use request::{BadRequestError, MethodNotAllowedError, Request, RequestError};
pub use response::Response;

use crate::message::Message;
use crate::response::{InternalServerError, NotFoundError, ResponseError};
pub fn handle_handshake(fd: RawFd) -> () {
    let mut buf = [0u8; 1024];
    let n = recv(fd, &mut buf, MsgFlags::empty()).unwrap();

    if n == 0 {
        println!("Message empty");
    }

    let request_string = String::from_utf8_lossy(&buf[0..n]).to_string();

    let request = Request::try_from(request_string.as_str());

    let mut response: Result<Response, ResponseError>;

    let response = match request {
        Ok(request) => Response::try_from(&request),
        Err(err) => {
            send(fd, err.to_string().as_bytes(), MsgFlags::empty());
            return;
        }
    };

    let response_string = match response {
        Ok(response) => response.to_string(),
        Err(ResponseError::NotFoundError) => NotFoundError.to_string(),
        Err(ResponseError::InternalServerError) => InternalServerError.to_string(),
    };

    send(fd, response_string.as_bytes(), MsgFlags::empty());
}

pub fn handle_session(fd: RawFd) -> () {
    loop {
        let message = Message::from(fd);

        println!("{:?}", message);
    }
}
pub fn run() {
    let sock_addr = SockaddrIn::from_str("0.0.0.0:3000").unwrap();

    let fd = socket(
        AddressFamily::Inet,
        SockType::Stream,
        SockFlag::empty(),
        None,
    )
    .unwrap();

    setsockopt(&fd, ReuseAddr, &true).unwrap();

    bind(fd.as_raw_fd(), &sock_addr).unwrap();

    listen(&fd, Backlog::new(128).unwrap()).unwrap();

    let mut client_fd: i32;

    loop {
        client_fd = accept(fd.as_raw_fd()).unwrap();

        println!(
            "IPV4 Address: {:?}, Port: {:?}",
            &sock_addr.ip(),
            &sock_addr.port()
        );
        println!("File descriptor: {:?}", &fd);
        handle_handshake(client_fd);

        handle_session(client_fd);

        close(client_fd).unwrap();
    }
}
