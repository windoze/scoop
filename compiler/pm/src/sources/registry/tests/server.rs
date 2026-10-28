use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

pub(super) struct Server {
    pub url: String,
    routes: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    requests: Arc<Mutex<Vec<String>>>,
    stopping: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let routes: Arc<Mutex<BTreeMap<String, Vec<u8>>>> = Arc::default();
        let requests: Arc<Mutex<Vec<String>>> = Arc::default();
        let stopping = Arc::new(AtomicBool::new(false));
        let (served, requested, stopped) = (routes.clone(), requests.clone(), stopping.clone());
        let thread = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap().to_owned();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                requested.lock().unwrap().push(path.clone());
                let body = served.lock().unwrap().get(&path).cloned();
                let status = if body.is_some() {
                    "200 OK"
                } else {
                    "404 Not Found"
                };
                let body = body.unwrap_or_default();
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        Self {
            url: format!("http://{address}/"),
            routes,
            requests,
            stopping,
            thread: Some(thread),
        }
    }

    pub fn set(&self, path: &str, bytes: Vec<u8>) {
        self.routes.lock().unwrap().insert(path.into(), bytes);
    }
    pub fn count(&self, path: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| *request == path)
            .count()
    }
    pub fn total(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        TcpStream::connect(self.url.trim_start_matches("http://").trim_end_matches('/')).unwrap();
        self.thread.take().unwrap().join().unwrap();
    }
}
