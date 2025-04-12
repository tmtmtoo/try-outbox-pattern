#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;

    let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;

    listener.listen("outbox_channel").await?;

    let listener_stream = {
        let stream = listener.into_stream();
        futures_util::StreamExt::map(stream, |r| r.map(|_|()))
    };

    let timer_stream = {
        let duration = tokio::time::Duration::from_secs(5);
        let interval = tokio::time::interval(duration);
        let stream = tokio_stream::wrappers::IntervalStream::new(interval);
        futures_util::StreamExt::map(stream, |_| Ok(()))
    };

    let mut combined_stream = tokio_stream::StreamExt::merge(listener_stream, timer_stream);

    while let Ok(Some(_)) = futures_util::TryStreamExt::try_next(&mut combined_stream).await {
        let _ = process_outbox(&pool).await;
    }

    Ok(())
}

#[derive(Debug)]
struct Outbox {
    id: i64,
    topic: String,
    payload: String,
}

async fn process_outbox(
    pool: &sqlx::Pool<sqlx::Postgres>
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = pool.begin().await?;

    let list = sqlx::query_as!(
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
        for update
            skip locked
    "#
    )
    .fetch_all(&mut *tx)
    .await?;

    for outbox in list {
        println!("Processing outbox: {:?}", outbox);

        sqlx::query!(
            r#"
            update outbox
            set processed_at = now()
            where id = $1
        "#,
            outbox.id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}
