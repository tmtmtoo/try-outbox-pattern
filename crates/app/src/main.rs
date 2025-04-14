#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;

    use sqlx::Connection;
    let mut conn = sqlx::postgres::PgConnection::connect(&database_url).await?;

    let (post, event) = post("Hello, world!", "This is my post.");

    {
        let mut tx = conn.begin().await?;

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
