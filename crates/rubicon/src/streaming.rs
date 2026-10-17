#[cfg(feature = "streaming")]
use deno_core::JsRuntime;
use deno_core::{OpState, futures::Stream, op2};
use std::{
    cell::RefCell,
    collections::HashMap,
    io,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) const CHUNK_BYTES: usize = 64 * 1024;
const QUEUED_CHUNKS: usize = 2;
pub(crate) type StreamCompletion = oneshot::Receiver<Result<(), String>>;

#[derive(Clone, Debug)]
enum Signal {
    Active,
    Finished,
    Cancelled,
    Failed,
}

/// Rust-owned chunks with backpressure. Dropping the consumer cancels the JS reader.
#[derive(Debug)]
pub struct RenderStream {
    receiver: mpsc::Receiver<Result<Vec<u8>, String>>,
    signal: watch::Sender<Signal>,
    terminal: bool,
}

impl Stream for RenderStream {
    type Item = Result<Vec<u8>, io::Error>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminal {
            return Poll::Ready(None);
        }
        match self.receiver.poll_recv(cx) {
            Poll::Ready(Some(Ok(bytes))) => Poll::Ready(Some(Ok(bytes))),
            Poll::Ready(Some(Err(error))) => {
                self.terminal = true;
                Poll::Ready(Some(Err(io::Error::other(error))))
            }
            Poll::Ready(None) => {
                self.terminal = true;
                let failed = matches!(*self.signal.borrow(), Signal::Failed);
                Poll::Ready(failed.then(|| {
                    Err(io::Error::other(
                        "Renderer stopped before stream completion",
                    ))
                }))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for RenderStream {
    fn drop(&mut self) {
        self.signal.send_replace(Signal::Cancelled);
    }
}

struct Entry {
    sender: Option<mpsc::Sender<Result<Vec<u8>, String>>>,
    output: Option<RenderStream>,
    #[cfg(feature = "streaming")]
    completion: Option<StreamCompletion>,
    completed: Option<oneshot::Sender<Result<(), String>>>,
    signal: watch::Sender<Signal>,
}

impl Drop for Entry {
    fn drop(&mut self) {
        if self.completed.is_some() {
            self.signal.send_replace(Signal::Failed);
        }
    }
}

#[derive(Default)]
struct Streams {
    next: u32,
    entries: HashMap<u32, Entry>,
}

#[op2(fast)]
fn op_rubicon_stream_open(state: &mut OpState) -> u32 {
    let streams = state.borrow_mut::<Streams>();
    let Some(id) = streams.next.checked_add(1) else {
        return 0;
    };
    streams.next = id;
    let (sender, receiver) = mpsc::channel(QUEUED_CHUNKS);
    let (completed, _completion) = oneshot::channel();
    let (signal, _) = watch::channel(Signal::Active);
    streams.entries.insert(
        id,
        Entry {
            sender: Some(sender),
            output: Some(RenderStream {
                receiver,
                signal: signal.clone(),
                terminal: false,
            }),
            #[cfg(feature = "streaming")]
            completion: Some(_completion),
            completed: Some(completed),
            signal,
        },
    );
    id
}

#[op2]
async fn op_rubicon_stream_write(
    state: Rc<RefCell<OpState>>,
    id: u32,
    #[buffer(copy)] bytes: Vec<u8>,
) -> u32 {
    if bytes.len() > CHUNK_BYTES {
        return 2;
    }
    let sender = {
        let state = state.borrow();
        state
            .borrow::<Streams>()
            .entries
            .get(&id)
            .and_then(|entry| entry.sender.clone())
    };
    match sender {
        Some(sender) if sender.send(Ok(bytes)).await.is_ok() => 1,
        _ => 0,
    }
}

#[op2]
async fn op_rubicon_stream_closed(state: Rc<RefCell<OpState>>, id: u32) -> bool {
    let mut signal = {
        let state = state.borrow();
        let Some(entry) = state.borrow::<Streams>().entries.get(&id) else {
            return true;
        };
        entry.signal.subscribe()
    };
    loop {
        match *signal.borrow_and_update() {
            Signal::Cancelled | Signal::Failed => return true,
            Signal::Finished => return false,
            Signal::Active => {}
        }
        if signal.changed().await.is_err() {
            return true;
        }
    }
}

#[op2]
async fn op_rubicon_stream_finish(
    state: Rc<RefCell<OpState>>,
    id: u32,
    #[string] error: Option<String>,
) {
    let sender = {
        let mut state = state.borrow_mut();
        state
            .borrow_mut::<Streams>()
            .entries
            .get_mut(&id)
            .and_then(|entry| entry.sender.take())
    };
    if let (Some(sender), Some(error)) = (sender, error.as_ref()) {
        let _ = sender.send(Err(error.clone())).await;
    }
    let mut state = state.borrow_mut();
    let streams = state.borrow_mut::<Streams>();
    let remove = if let Some(entry) = streams.entries.get_mut(&id) {
        entry.signal.send_replace(Signal::Finished);
        if let Some(completed) = entry.completed.take() {
            let _ = completed.send(error.map_or(Ok(()), Err));
        }
        entry.output.is_none()
    } else {
        false
    };
    if remove {
        streams.entries.remove(&id);
    }
}

#[cfg(feature = "streaming")]
pub(crate) fn take(
    runtime: &mut JsRuntime,
    id: u32,
) -> Result<(RenderStream, StreamCompletion), String> {
    let state = runtime.op_state();
    let mut state = state.borrow_mut();
    let streams = state.borrow_mut::<Streams>();
    let entry = streams
        .entries
        .get_mut(&id)
        .ok_or("Unknown response stream")?;
    let output = entry
        .output
        .take()
        .ok_or("Response stream already consumed")?;
    let completion = entry
        .completion
        .take()
        .ok_or("Response completion already consumed")?;
    if entry.completed.is_none() {
        streams.entries.remove(&id);
    }
    Ok((output, completion))
}

deno_core::extension!(rubicon_streaming,
    deps = [rubicon_web],
    ops = [op_rubicon_stream_open, op_rubicon_stream_write, op_rubicon_stream_closed, op_rubicon_stream_finish],
    esm_entry_point = "ext:rubicon_streaming/streaming.js",
    esm = [dir "js", "streaming.js"],
    state = |state| { state.put(Streams::default()); },
);

#[cfg(feature = "streaming")]
pub(crate) fn extension() -> deno_core::Extension {
    rubicon_streaming::init()
}
