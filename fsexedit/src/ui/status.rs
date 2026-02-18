use std::time::Instant;

pub struct StatusMessage {
  pub text: String,
  pub created: Instant,
}

impl StatusMessage {
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      text: text.into(),
      created: Instant::now(),
    }
  }

  pub fn is_expired(&self) -> bool {
    self.created.elapsed().as_secs() > 3
  }
}
