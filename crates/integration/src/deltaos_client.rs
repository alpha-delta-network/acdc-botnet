/// DeltaOS REST API client
// Placeholder for Phase 1 task #4

pub struct DeltaOSClient {
    base_url: String,
    client: reqwest::Client,
}

impl DeltaOSClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}
