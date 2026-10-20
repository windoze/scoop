use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

pub(crate) enum Message {
    Output(Vec<u8>),
    End(io::Result<()>),
}

pub(crate) fn run(
    mut reader: Box<dyn Read + Send>,
    fd: libc::c_int,
    stop: Arc<AtomicBool>,
    sender: Sender<Message>,
) {
    let result = read_loop(&mut reader, fd, &stop, &sender);
    let _ = sender.send(Message::End(result));
}

fn read_loop(
    reader: &mut dyn Read,
    fd: libc::c_int,
    stop: &AtomicBool,
    sender: &Sender<Message>,
) -> io::Result<()> {
    let mut bytes = [0; 4096];
    while !stop.load(Ordering::Relaxed) {
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // The host owns the master until this reader has joined.
        let ready = unsafe { libc::poll(&mut poll, 1, 50) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if ready == 0 {
            continue;
        }
        match reader.read(&mut bytes) {
            Ok(0) => return Ok(()),
            Ok(count) => {
                if sender
                    .send(Message::Output(bytes[..count].to_vec()))
                    .is_err()
                {
                    return Ok(());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            // portable-pty translates Linux's slave-closed EIO into EOF.
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
