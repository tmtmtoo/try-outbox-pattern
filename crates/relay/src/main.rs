use futures_util::TryStreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;

    let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;

    listener.listen("outbox_channel").await?;

    let mut stream = listener.into_stream();

    while let Ok(Some(_)) = stream.try_next().await {
        match sqlx::query_as!(
            Outbox,
            r#"
            select
                id,
                topic,
                payload
            from
                outbox
            where
                processed_at is null
            order by
                created_at asc                
        "#
        )
        .fetch_all(&pool)
        .await
        {
            Ok(list) => {}
            Err(_) => continue,
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Outbox {
    id: i64,
    topic: String,
    payload: String,
}
