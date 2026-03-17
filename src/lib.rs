use nix::sys::socket::{SockaddrIn, SockaddrLike, getpeername};
use nix::sys::socket::{
    AddressFamily, Backlog, MsgFlags, SockFlag, SockType, accept, bind, listen, recv,
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
use crate::message::FrameError;
pub use crate::message::{ClientMessage, Frame, ServerMessage};

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Debug)]
struct ConnState {
    addr: SockaddrIn,
    connected_at: std::time::Instant,
}

type ConnMap = Arc<Mutex<HashMap<RawFd, ConnState>>>;

#[derive(Debug)]
pub struct SessionError; 

pub fn handle_handshake(fd: RawFd, conns: &ConnMap) -> Result<(), SessionError> {
    let mut buf = [0u8; 1024];
    let n = recv(fd, &mut buf, MsgFlags::empty()).unwrap();

    if n == 0 {
        println!("Message empty");
    }

    let request_string = String::from_utf8_lossy(&buf[0..n]).to_string();

    let request_wrapped = Request::try_from(request_string.as_str());

    let response = match request_wrapped {
        Ok(request) => Response::try_from(&request),
        Err(_) => {
            return Err(SessionError);
        }, 
    };

    match response {
        Ok(response) => {
            let response_string = response.to_string();
            send(fd, response_string.as_bytes(), MsgFlags::empty());
            return Ok(())
        }, 
        Err(ResponseError::NotFoundError) => {
            let response_string = NotFoundError.to_string();
            send(fd, response_string.as_bytes(), MsgFlags::empty());
            return Err(SessionError);
        },  
        Err(ResponseError::InternalServerError) => {
            let response_string = InternalServerError.to_string();
            send(fd, response_string.as_bytes(), MsgFlags::empty());
            return Err(SessionError);
        },
    };
}

pub fn handle_session(fd: RawFd, conns: &ConnMap) -> Result<(), SessionError> {

    type Error = SessionError; 
    loop {
        let client_message = ClientMessage::from(fd).unwrap();
        // println!("{:?}", client_message);
        
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
                    // conns.lock().unwrap().remove(&fd);
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
            
            {
                let guard = conns.lock().unwrap();

                for (key, value) in guard.iter() {
                    println!("{}: {:?}", key, value.addr.to_string());
                }

            }

            break;


        }; 
    }

    return Ok(())
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
    
    let connections: ConnMap = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let mut client_fd: i32 = fd.as_raw_fd();

        let connections = Arc::clone(&connections);
    
        client_fd = accept(client_fd).unwrap();

        let client_addr = getpeername(client_fd).unwrap(); 

        connections.lock().unwrap().insert(client_fd, ConnState {
            addr: client_addr,
            connected_at: std::time::Instant::now(),
        });
    
        // println!(
        //     "IPV4 Address: {:?}, Port: {:?}",
        //     &sock_addr.ip(),
        //     &sock_addr.port()
        // );
        // println!("File descriptor: {:?}", &fd);

        thread::spawn(move || {

            {
                let guard = connections.lock().unwrap();

                for (key, value) in guard.iter() {
                    println!("{}: {:?}", key, value.addr.to_string());
                }

            }

            match handle_handshake(client_fd, &connections){
                Ok(_) => println!("{} http handshake successful", client_fd),
                Err(_) => {
                    connections.lock().unwrap().remove(&client_fd);
                    println!("Error in handshake. {} disconnected", client_fd);
                },
            };
            
            match handle_session(client_fd, &connections) {
                Ok(_) => {
                    connections.lock().unwrap().remove(&client_fd);
                    println!("{} safely disconnected", client_fd); 
                },
                Err(_) => {
                    connections.lock().unwrap().remove(&client_fd);
                    println!("Error in session. {} disconnected", client_fd); 
                },
            };
        });
    }
}
