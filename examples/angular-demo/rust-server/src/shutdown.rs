use std::{future::Future, io, thread};

use tokio::sync::oneshot;

pub fn signal() -> Result<impl Future<Output = Result<(), String>>, String> {
    let input_closed = match std::env::var_os("RUBICON_MANAGED") {
        None => None,

        Some(value) if value == "1" => {
            let (sender, receiver) = oneshot::channel();

            let _input_thread = thread::Builder::new()
                .name("rubicon-shutdown-input".to_owned())
                .spawn(move || {
                    let result = io::copy(&mut io::stdin().lock(), &mut io::sink())
                        .map(|_| ())
                        .map_err(|error| format!("Shutdown input failed: {error}"));

                    let _ = sender.send(result);
                })
                .map_err(|error| format!("Shutdown monitor creation failed: {error}"))?;

            Some(receiver)
        }

        Some(_) => {
            return Err("RUBICON_MANAGED must be 1 when set.".to_owned());
        }
    };

    Ok(async move {
        match input_closed {
            None => wait_for_ctrl_c().await,

            Some(receiver) => {
                tokio::select! {
                    result = wait_for_ctrl_c() => result,

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

async fn wait_for_ctrl_c() -> Result<(), String> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|error| format!("Shutdown signal failed: {error}"))
}
