#[derive(Debug, serde::Deserialize)]
struct Config {
    database_url: String,
    connection_count: u32,
    post_count: u32,
}

impl Config {
    fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        envy::from_env().map_err(Into::into)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.connection_count)
        .connect(&config.database_url)
        .await
        .map(std::sync::Arc::new)?;

    for _ in 0..config.post_count {
        let pool = pool.clone();
        tokio::spawn(async move {
            let (post, event) = post("Hello, world!", "This is my post.");
            if let Err(e) = insert_post(&pool, &post, &event).await {
                eprintln!("Failed to insert post: {e}");
            }
        });
    }

    Ok(())
}

struct Post<'a> {
    id: uuid::Uuid,
    title: &'a str,
    content: &'a str,
}

fn post<'a>(title: &'a str, content: &'a str) -> (Post<'a>, event::Event<event::Posted>) {
    let id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now().naive_utc();
    let event = event::Event::new(
        event::Posted {
            id: id.clone(),
            title: title.into(),
        },
        now,
    );
    let post = Post { id, title, content };

    (post, event)
}

async fn insert_post(
    pool: &sqlx::Pool<sqlx::Postgres>,
    post: &Post<'_>,
    event: &event::Event<event::Posted>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "insert into post (id, title, content) values ($1, $2, $3)",
        post.id,
        post.title,
        post.content
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "insert into outbox (topic, payload, created_at) values ($1, $2, $3)",
        event.topic(),
        event.payload(),
        event.created_at()
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(())
}
