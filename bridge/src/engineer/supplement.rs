//! Conservative, read-only REST supplement. No account or RaceControl requests.
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
pub struct Supplement {
    pub active: Arc<AtomicBool>,
    pub value: Arc<Mutex<Option<Value>>>,
}
impl Default for Supplement {
    fn default() -> Self {
        Self::new()
    }
}
impl Supplement {
    pub fn new() -> Self {
        let active = Arc::new(AtomicBool::new(false));
        let value = Arc::new(Mutex::new(None));
        let a = active.clone();
        let v = value.clone();
        std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_millis(200))
                .timeout_read(Duration::from_millis(350))
                .timeout_write(Duration::from_millis(200))
                .build();
            let mut failures = 0u32;
            loop {
                if !a.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
                let mut data = json!({"observed_ms":super::model::now_ms()});
                let mut count = 0;
                for (name, path) in [
                    ("weather", "/rest/sessions/weather"),
                    ("wearables", "/rest/garage/UIScreen/RepairAndRefuel"),
                    ("pit_estimate", "/rest/strategy/pitstop-estimate"),
                ] {
                    if let Ok(r) = agent.get(&format!("http://127.0.0.1:6397{path}")).call() {
                        use std::io::Read;
                        let mut bytes = Vec::new();
                        if r.into_reader()
                            .take(2 * 1024 * 1024)
                            .read_to_end(&mut bytes)
                            .is_ok()
                        {
                            if let Ok(response) = serde_json::from_slice::<Value>(&bytes) {
                                if !response.is_null() {
                                    data[name] = response;
                                    count += 1;
                                }
                            }
                        }
                    }
                }
                if count > 0 {
                    *v.lock().unwrap_or_else(|e| e.into_inner()) = Some(data);
                    failures = 0
                } else {
                    failures = (failures + 1).min(5)
                }
                std::thread::sleep(Duration::from_secs(if failures > 0 {
                    30 * failures as u64
                } else {
                    5
                }));
            }
        });
        Self { active, value }
    }
    pub fn read(&self, active: bool) -> Option<Value> {
        self.active.store(active, Ordering::Relaxed);
        self.value.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
