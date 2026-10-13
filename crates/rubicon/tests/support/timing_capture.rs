use std::sync::{Arc, Mutex};

use tracing::{
    Event, Metadata, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
};

#[derive(Clone, Default)]
pub struct TimingCapture(Arc<Mutex<Vec<(String, String)>>>);

impl TimingCapture {
    pub fn stages(&self, request_id: &str) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| id == request_id)
            .map(|(_, stage)| stage.clone())
            .collect()
    }
}

#[derive(Default)]
struct Fields {
    request_id: Option<String>,
    stage: Option<String>,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "request_id" => self.request_id = Some(value.to_owned()),
            "stage" => self.stage = Some(value.to_owned()),
            _ => {}
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}

impl Subscriber for TimingCapture {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _attributes: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _span: &Id, _values: &Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
    fn enter(&self, _span: &Id) {}
    fn exit(&self, _span: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if event.metadata().target() != "rubicon::adapter_timing" {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        if let (Some(id), Some(stage)) = (fields.request_id, fields.stage) {
            self.0.lock().unwrap().push((id, stage));
        }
    }
}
