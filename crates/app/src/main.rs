#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;

    let mut tx = pool.begin().await?;

    let (post, event) = post("Hello, world!", "This is my post.");

    sqlx::query("insert into post (id, title, content) values ($1, $2, $3)")
        .bind(post.id)
        .bind(post.title)
        .bind(post.content)
        .execute(&mut *tx)
        .await?;

    sqlx::query("insert into outbox (topic, payload, created_at) values ($1, $2, $3)")
        .bind(event.topic.to_string())
        .bind(event.payload_string()?)
        .bind(event.created_at)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

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

    let event = event::Event {
        topic: event::Topic::Post,
        payload: event::Posted {
            id: id.clone(),
            title: title.to_string(),
            posted_at: now.clone(),
        },
        created_at: now,
    };

    let post = Post { id, title, content };

    (post, event)
}
