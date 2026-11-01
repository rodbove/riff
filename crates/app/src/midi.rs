use std::sync::mpsc::{Receiver, Sender, channel};

use midir::{MidiInput, MidiInputConnection};

const CLIENT: &str = "riff";
pub const EXPRESSION: u8 = 11;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Program(u8),
    Control { cc: u8, value: u8 },
}

pub fn parse(bytes: &[u8]) -> Option<Event> {
    let (&status, rest) = bytes.split_first()?;
    match (status & 0xF0, rest) {
        (0xC0, [program, ..]) => Some(Event::Program(*program)),
        (0xB0, [cc, value, ..]) => Some(Event::Control {
            cc: *cc,
            value: *value,
        }),
        _ => None,
    }
}

pub fn ports() -> Vec<String> {
    let Ok(input) = MidiInput::new(CLIENT) else {
        return Vec::new();
    };
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .collect()
}

pub struct Midi {
    connection: Option<MidiInputConnection<()>>,
    pub port: Option<String>,
    events: Receiver<Event>,
    sender: Sender<Event>,
}

impl Default for Midi {
    fn default() -> Self {
        let (sender, events) = channel();
        Self {
            connection: None,
            port: None,
            events,
            sender,
        }
    }
}

impl Midi {
    pub fn connect(
        &mut self,
        wanted: &str,
        repaint: impl Fn() + Send + 'static,
    ) -> Result<(), String> {
        self.disconnect();
        let input = MidiInput::new(CLIENT).map_err(|e| e.to_string())?;
        let port = input
            .ports()
            .into_iter()
            .find(|p| input.port_name(p).is_ok_and(|n| n == wanted))
            .ok_or("port not found")?;
        let sender = self.sender.clone();
        let connection = input
            .connect(
                &port,
                "riff-in",
                move |_, bytes, _| {
                    if let Some(e) = parse(bytes) {
                        let _ = sender.send(e);
                        repaint();
                    }
                },
                (),
            )
            .map_err(|e| e.to_string())?;
        self.connection = Some(connection);
        self.port = Some(wanted.to_string());
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(c) = self.connection.take() {
            c.close();
        }
        self.port = None;
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_parsing {
        use super::*;

        #[test]
        fn reads_program_changes_and_controls() {
            assert_eq!(parse(&[0xC2, 5]), Some(Event::Program(5)));
            assert_eq!(
                parse(&[0xB0, EXPRESSION, 90]),
                Some(Event::Control { cc: 11, value: 90 })
            );
            assert_eq!(parse(&[0x90, 60, 100]), None);
        }
    }
}
