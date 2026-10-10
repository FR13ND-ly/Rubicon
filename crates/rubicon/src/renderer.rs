use std::{path::PathBuf, thread};

use tokio::sync::{mpsc, oneshot};

use crate::{JavaScriptRuntime, RenderRequest};

const QUEUE_CAPACITY: usize = 16;

struct RenderJob {
    request: RenderRequest,
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

    pub async fn render(&self, request: RenderRequest) -> Result<String, RenderError> {
        let (reply, response) = oneshot::channel();

        self.sender
            .try_send(RenderJob { request, reply })
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

            let result = runtime
                .call_string_export(module_id, "render", &job.request)
                .await;

            let _ = job.reply.send(result);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{QUEUE_CAPACITY, RenderError, RenderJob, Renderer};
    use crate::RenderRequest;
    use tokio::sync::{mpsc, oneshot};

    #[tokio::test]
    async fn full_queue_rejects_render_without_waiting_for_capacity() {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let renderer = Renderer { sender };
        let mut pending_replies = Vec::new();

        for _ in 0..QUEUE_CAPACITY {
            let request = RenderRequest {
                url: "http://127.0.0.1:3000/test".to_owned(),
                method: "GET".to_owned(),
                headers: Vec::new(),
            };
            let (reply, response) = oneshot::channel();

            assert!(
                renderer
                    .sender
                    .try_send(RenderJob { request, reply })
                    .is_ok()
            );
            pending_replies.push(response);
        }

        let result = tokio::time::timeout(
            Duration::from_secs(2),
            renderer.render(RenderRequest {
                url: "http://127.0.0.1:3000/test".to_owned(),
                method: "GET".to_owned(),
                headers: Vec::new(),
            }),
        )
        .await
        .expect("A full queue must reject the request without waiting for capacity");

        assert!(matches!(result, Err(RenderError::Busy)));
        assert_eq!(receiver.len(), QUEUE_CAPACITY);
    }
}
