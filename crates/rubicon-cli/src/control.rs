use std::{future::Future, io, thread};

use tokio::sync::oneshot;

pub fn signal(managed: bool) -> Result<impl Future<Output = Result<(), String>>, String> {
    #[cfg(windows)]
    let mut interrupt = tokio::signal::windows::ctrl_c()
        .map_err(|error| format!("Shutdown signal initialization failed: {error}"))?;

    #[cfg(unix)]
    let mut interrupt =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .map_err(|error| format!("Shutdown signal initialization failed: {error}"))?;

    let input_closed = if managed {
        let (sender, receiver) = oneshot::channel();

        let _input_thread = thread::Builder::new()
            .name("rb-shutdown-input".to_owned())
            .spawn(move || {
                let result = io::copy(&mut io::stdin().lock(), &mut io::sink())
                    .map(|_| ())
                    .map_err(|error| format!("Shutdown input failed: {error}"));

                let _ = sender.send(result);
            })
            .map_err(|error| format!("Shutdown monitor creation failed: {error}"))?;

        Some(receiver)
    } else {
        None
    };

    Ok(async move {
        let interrupted = async move {
            interrupt
                .recv()
                .await
                .ok_or_else(|| "Shutdown signal stream closed.".to_owned())
        };

        match input_closed {
            None => interrupted.await,

            Some(receiver) => {
                tokio::select! {
                    result = interrupted => result,

                    result = receiver => {
                        result.map_err(|_| {
                            "Shutdown input monitor stopped unexpectedly.".to_owned()
                        })?
                    }
                }
            }
        }
    })
}
