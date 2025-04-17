pub mod topic {
    pub const POST: &'static str = "post";
}

pub trait Topic {
    fn topic(&self) -> &'static str;
}

pub struct Event<Payload> {
    payload: Payload,
    created_at: chrono::NaiveDateTime,
}

impl<Payload> Event<Payload>
where
    Payload: std::fmt::Display,
{
    pub fn payload(&self) -> String {
        self.payload.to_string()
    }
}

impl<Payload> Event<Payload>
where
    Payload: Topic,
{
    pub fn topic<'a>(&'a self) -> &'a str {
        self.payload.topic()
    }
}

impl<Payload> Event<Payload> {
    pub fn new(payload: Payload, created_at: chrono::NaiveDateTime) -> Self {
        Self {
            payload,
            created_at,
        }
    }

    pub fn created_at<'a>(&'a self) -> &'a chrono::NaiveDateTime {
        &self.created_at
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Posted {
    pub id: uuid::Uuid,
    pub title: String,
}

impl std::fmt::Display for Posted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{json}")
    }
}

impl Topic for Posted {
    fn topic(&self) -> &'static str {
        topic::POST
    }
}
