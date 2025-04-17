#[derive(Debug, serde::Deserialize)]
struct Config {
    database_url: String,
    app_database_max_connections: u32,
    app_posts: u32,
}

// todo: 設定としてrpsを持ち、その頻度に応じてpostを行う

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = envy::from_env::<Config>()?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.app_database_max_connections)
        .connect(&config.database_url)
        .await
        .map(std::sync::Arc::new)?;

    let mut handles = Vec::with_capacity(config.app_posts as usize);

    for _ in 0..config.app_posts {
        let pool = pool.clone();
        let handle = tokio::spawn(async move {
            let (post, event) = post("Hello, world!", "This is my post.");
            if let Err(e) = insert_post(&pool, &post, &event).await {
                eprintln!("Failed to insert post: {e}");
            }
        });
        handles.push(handle);
    }

    futures_util::future::try_join_all(handles).await?;

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
