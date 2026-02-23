/// AlphaOS REST API client
// Placeholder for Phase 1 task #4

pub struct AlphaOSClient {
    base_url: String,
    client: reqwest::Client,
}

impl AlphaOSClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}
