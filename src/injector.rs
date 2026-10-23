use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

/// Runs text replacements one at a time, in submission order, off the event-tap thread.
pub struct Injector {
    jobs: Sender<(usize, String)>,
}

impl Injector {
    /// `delay` lets the key that ended the word reach the app before each replacement starts.
    pub fn new(delay: Duration, mut sink: impl FnMut(usize, &str) + Send + 'static) -> Injector {
        let (jobs, queue) = mpsc::channel::<(usize, String)>();
        thread::spawn(move || {
            for (backspaces, text) in queue {
                thread::sleep(delay);
                sink(backspaces, &text);
            }
        });
        Injector { jobs }
    }

    pub fn submit(&self, backspaces: usize, text: String) {
        let _ = self.jobs.send((backspaces, text));
    }
}

