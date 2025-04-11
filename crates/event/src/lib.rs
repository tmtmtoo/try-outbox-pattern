#[derive(serde::Serialize, serde::Deserialize)]
pub enum Topic {
    Post,
}

impl std::fmt::Display for Topic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Topic::Post => write!(f, "post"),
        }
    }
}

pub trait TryIntoPayloadString {
    fn try_into_payload_string(&self) -> Result<String, Box<dyn std::error::Error>>;
}

pub struct Event<T>
where
    T: TryIntoPayloadString,
{
    pub topic: Topic,
    pub payload: T,
    pub created_at: chrono::NaiveDateTime,
}

impl<T> Event<T>
where
    T: TryIntoPayloadString,
{
    pub fn payload_string(&self) -> Result<String, Box<dyn std::error::Error>> {
        self.payload.try_into_payload_string()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Posted {
    pub id: uuid::Uuid,
    pub title: String,
    pub posted_at: chrono::NaiveDateTime,
}

impl TryIntoPayloadString for Posted {
    fn try_into_payload_string(&self) -> Result<String, Box<dyn std::error::Error>> {
        let payload = serde_json::to_string(self)?;
        Ok(payload)
    }
}
