use libmpv2::{events::Event, mpv_end_file_reason, Mpv};
use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
};

enum Command {
    Play(String),
    Shutdown,
}

pub struct Player {
    tx: Sender<Command>,
}

impl Player {
    pub fn new(on_error: impl Fn(String) + Send + 'static) -> Player {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || run(rx, on_error));
        Player { tx }
    }

    pub fn play(&self, url: String) {
        let _ = self.tx.send(Command::Play(url));
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

fn create() -> libmpv2::Result<Mpv> {
    Mpv::with_initializer(|init| {
        init.set_option("force-window", "yes")?;
        init.set_option("osc", "yes")?;
        init.set_option("input-default-bindings", "yes")?;
        init.set_option("input-vo-keyboard", "yes")
    })
}

fn run(rx: Receiver<Command>, on_error: impl Fn(String)) {
    let mut mpv: Option<Mpv> = None;
    loop {
        let command = if mpv.is_some() {
            match rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        } else {
            match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            }
        };

        match command {
            Some(Command::Shutdown) => return,
            Some(Command::Play(url)) => {
                if mpv.is_none() {
                    match create() {
                        Ok(m) => mpv = Some(m),
                        Err(e) => {
                            on_error(format!("Initialisation de mpv impossible : {e}"));
                            continue;
                        }
                    }
                }
                if let Some(Err(e)) = mpv.as_ref().map(|m| m.command("loadfile", &[&url, "replace"])) {
                    on_error(format!("Lecture impossible : {e}"));
                }
            }
            None => {}
        }

        let Some(m) = &mpv else { continue };
        let destroy = match m.wait_event(0.1) {
            Some(Ok(Event::Shutdown)) => true,
            Some(Ok(Event::EndFile(reason))) => {
                reason == mpv_end_file_reason::Eof || reason == mpv_end_file_reason::Quit
            }
            Some(Err(e)) => {
                on_error(format!("Lecture impossible : média introuvable ou illisible ({e:?})"));
                true
            }
            _ => false,
        };
        if destroy {
            mpv = None;
        }
    }
}
