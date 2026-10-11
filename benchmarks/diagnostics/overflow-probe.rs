// Standalone diagnostic: compile with the same pinned tracing dependencies.
use std::{
    io::{self, Write},
    sync::{Arc, Condvar, Mutex, mpsc},
    time::Duration,
};

struct HeldWriter {
    release: Arc<(Mutex<bool>, Condvar)>,
    entered: Option<mpsc::Sender<()>>,
    output: Arc<Mutex<Vec<u8>>>,
}

impl Write for HeldWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Some(entered) = self.entered.take() {
            entered.send(()).expect("Probe observer stopped");
            let (released, wake) = &*self.release;
            let released = released.lock().expect("Probe gate poisoned");
            drop(
                wake.wait_while(released, |value| !*value)
                    .expect("Probe gate poisoned"),
            );
        }
        self.output
            .lock()
            .expect("Probe output poisoned")
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn main() {
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let output = Arc::new(Mutex::new(Vec::new()));
    let (entered, waiting) = mpsc::channel();
    let sink = HeldWriter {
        release: release.clone(),
        entered: Some(entered),
        output: output.clone(),
    };
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(2)
        .lossy(true)
        .finish(sink);
    let dropped = writer.error_counter();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(request_id = 0, "render_timing");
        waiting
            .recv_timeout(Duration::from_secs(2))
            .expect("Writer did not enter its gate");
        // Returning before the blocked sink is released establishes that this
        // fixed burst does not wait for writer capacity; it is not a speed test.
        for request_id in 1..=64 {
            tracing::info!(request_id, "render_timing");
        }
        assert_eq!(dropped.dropped_lines(), 62);
    });
    *release.0.lock().expect("Probe gate poisoned") = true;
    release.1.notify_all();
    drop(guard);
    let bytes = output.lock().expect("Probe output poisoned");
    let text = std::str::from_utf8(&bytes).expect("Probe output was not UTF-8");
    assert_eq!(text.lines().count(), 3);
    assert_eq!(text.lines().count() + dropped.dropped_lines(), 65);
    println!("LOGGER_OVERFLOW_PASS emitted=65 recorded=3 dropped=62");
}
