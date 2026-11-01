use crate::{PmError, PmResult};

pub(super) fn get(http: &ureq::Agent, url: &str) -> PmResult<ureq::http::Response<ureq::Body>> {
    let response = match http.get(url).call() {
        Err(ureq::Error::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
            ) =>
        {
            // An idempotent GET may race the server closing a pooled connection.
            // Retry once on a fresh socket, before any response body is consumed.
            http.get(url)
                .config()
                .max_idle_age(std::time::Duration::ZERO)
                .build()
                .call()
        }
        result => result,
    };
    response.map_err(|error| PmError::new(format!("GET {url}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::{TcpListener, TcpStream},
        thread,
        time::Duration,
    };

    fn read_request(stream: &mut TcpStream) -> std::io::Result<bool> {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(false);
        }
        while line != "\r\n" {
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn client() -> ureq::Agent {
        ureq::Agent::config_builder()
            .proxy(None)
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .into()
    }

    #[test]
    fn a_closing_http10_connection_does_not_break_the_next_get() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            {
                let (mut first, _) = listener.accept().unwrap();
                assert!(read_request(&mut first).unwrap());
                first
                    .write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                    .unwrap();
                // Hold the closing connection open until the client tries to reuse it.
                // A client that already honors HTTP/1.0 closure may open a new socket.
                let _ = read_request(&mut first);
            }
            let (mut second, _) = listener.accept().unwrap();
            if read_request(&mut second).unwrap() {
                second
                    .write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\n[]")
                    .unwrap();
            }
        });
        let http = client();
        let url = format!("http://{address}/package.json");
        let results: PmResult<Vec<_>> = (0..2)
            .map(|_| {
                get(&http, &url)?
                    .body_mut()
                    .read_to_string()
                    .map_err(|error| PmError::new(error.to_string()))
            })
            .collect();
        // Wake accept if the failing client never opened its second connection.
        let _ = TcpStream::connect(address);
        server.join().unwrap();
        assert_eq!(results.unwrap(), ["{}", "[]"]);
    }

    #[test]
    fn a_disconnected_get_is_retried_only_once() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let mut requests = 0;
            for stream in listener.incoming() {
                if !read_request(&mut stream.unwrap()).unwrap() {
                    break;
                }
                requests += 1;
            }
            requests
        });
        let result = get(&client(), &format!("http://{address}/package.json"));
        let _ = TcpStream::connect(address);
        let requests = server.join().unwrap();
        assert!(result.is_err());
        assert_eq!(requests, 2);
    }

    #[test]
    fn status_errors_and_truncated_bodies_are_not_retried() {
        for response in [
            &b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"[..],
            &b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n{"[..],
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let mut requests = 0;
                for stream in listener.incoming() {
                    let mut stream = stream.unwrap();
                    if !read_request(&mut stream).unwrap() {
                        break;
                    }
                    requests += 1;
                    stream.write_all(response).unwrap();
                }
                requests
            });
            let result = get(&client(), &format!("http://{address}/package.json")).and_then(
                |mut response| {
                    response
                        .body_mut()
                        .read_to_string()
                        .map_err(|error| PmError::new(error.to_string()))
                },
            );
            let _ = TcpStream::connect(address);
            let requests = server.join().unwrap();
            assert!(result.is_err());
            assert_eq!(requests, 1);
        }
    }
}
