impl SystemProxy {
    pub fn new() -> Self {
        Self::Auto {
            url: "http://localhost:3129".to_string(),
        }
    }

    pub fn set_system_proxy(&self) {}
}
