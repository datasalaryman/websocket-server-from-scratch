use nix::sys::socket::{
    AddressFamily, Backlog, MsgFlags, SockFlag, SockType, SockaddrIn, accept, bind, listen, recv,
    send, setsockopt, socket, sockopt::ReuseAddr,
};
use nix::unistd::close;
use std::{os::fd::{AsRawFd, RawFd}, str::FromStr, thread};

pub mod http;
pub mod message;
pub use crate::http::{
    BadRequestError, InternalServerError, MethodNotAllowedError, NotFoundError, Request,
    RequestError, Response, ResponseError,
};
pub use crate::message::{ClientMessage, Frame, ServerMessage};
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
        let client_message = ClientMessage::from(fd);
        println!("{:?}", client_message);

        let server_message = ServerMessage::from(&client_message).unwrap();

        println!("{:?}", server_message);

        for frame in server_message.frames {
            let server_frame_bytes = Frame::as_bytes(frame.clone());

            println!(
                "{}",
                server_frame_bytes
                    .iter()
                    .map(|b| format!("{:08b}", b))
                    .collect::<String>()
            );

            match server_message.opcode {
                1 | 2 => {
                    println!("Sending payload back to client");
                    send(fd, &server_frame_bytes, MsgFlags::empty());
                }, 
                8 => {
                    println!("Client closed connection");
                    close(fd).unwrap();
                    break;
                }
                0x0A => {
                    println!("Sending pong");
                    send(fd, &server_frame_bytes, MsgFlags::empty());
                }
                _ => println!("handling cases later"),
            };

        };

        if server_message.opcode == 8 {
            break;
        }; 
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

        thread::spawn(move || {
            handle_handshake(client_fd);
            handle_session(client_fd);
        });
    }
}
