#[derive(Debug, serde::Deserialize)]
struct Config {
    database_url: String,
    producer_database_max_connections: u32,
    producer_rps: u32,
    producer_duration_secs: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use futures_util::StreamExt;

    let config = envy::from_env::<Config>()?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.producer_database_max_connections)
        .connect(&config.database_url)
        .await
        .map(std::sync::Arc::new)?;

    let ticker = {
        let duration = 1.0 / f64::from(config.producer_rps);
        let duration = tokio::time::Duration::from_secs_f64(duration);
        let interval = tokio::time::interval(duration);
        tokio_stream::wrappers::IntervalStream::new(interval)
    };

    let deadline = {
        let duration = f64::from(config.producer_duration_secs);
        let duration = tokio::time::Duration::from_secs_f64(duration);
        tokio::time::sleep(duration)
    };

    tokio::pin!(deadline);
    let mut timed_ticker = ticker.take_until(&mut deadline);

    let mut handles = {
        let capacity = config.producer_rps * config.producer_duration_secs;
        Vec::with_capacity(capacity as usize)
    };

    while let Some(_) = timed_ticker.next().await {
        let pool = pool.clone();
        let task = async move {
            let (post, event) = post("Hello, world!", "This is my post.");
            if let Err(e) = insert_post(&pool, &post, &event).await {
                eprintln!("Failed to insert post: {e}");
            }
        };
        let handle = tokio::spawn(task);
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
