use std::{path::PathBuf, thread};

use tokio::sync::{mpsc, oneshot};

use crate::JavaScriptRuntime;

const QUEUE_CAPACITY: usize = 16;

struct RenderJob {
    reply: oneshot::Sender<Result<String, String>>,
}

#[derive(Clone)]
pub struct Renderer {
    sender: mpsc::Sender<RenderJob>,
}

#[derive(Debug)]
pub enum RenderError {
    Busy,
    Unavailable,
    Failed(String),
}

impl Renderer {
    pub async fn start(module_path: PathBuf) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (ready_sender, ready_receiver) = oneshot::channel();

        let _worker = thread::Builder::new()
            .name("rubicon-renderer".to_owned())
            .spawn(move || run_worker(module_path, receiver, ready_sender))
            .map_err(|error| format!("Worker thread creation failed: {error}"))?;

        let startup_result = ready_receiver
            .await
            .map_err(|_| "Worker stopped during initialization".to_owned())?;

        startup_result?;

        Ok(Self { sender })
    }

    pub async fn render(&self) -> Result<String, RenderError> {
        let (reply, response) = oneshot::channel();

        self.sender
            .try_send(RenderJob { reply })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => RenderError::Busy,
                mpsc::error::TrySendError::Closed(_) => RenderError::Unavailable,
            })?;

        response
            .await
            .map_err(|_| RenderError::Unavailable)?
            .map_err(RenderError::Failed)
    }
}

fn run_worker(
    module_path: PathBuf,
    mut receiver: mpsc::Receiver<RenderJob>,
    ready: oneshot::Sender<Result<(), String>>,
) {
    let executor = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(executor) => executor,
        Err(error) => {
            let _ = ready.send(Err(format!("Worker executor creation failed: {error}")));
            return;
        }
    };

    executor.block_on(async move {
        let initialization = async {
            let mut runtime = JavaScriptRuntime::new()?;
            let module_id = runtime.load_main_module(&module_path).await?;

            Ok::<_, String>((runtime, module_id))
        }
        .await;

        let (mut runtime, module_id) = match initialization {
            Ok(initialized) => initialized,
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };

        if ready.send(Ok(())).is_err() {
            return;
        }

        while let Some(job) = receiver.recv().await {
            if job.reply.is_closed() {
                continue;
            }

            let result = runtime.call_string_export(module_id, "render").await;

            let _ = job.reply.send(result);
        }
    });
}
