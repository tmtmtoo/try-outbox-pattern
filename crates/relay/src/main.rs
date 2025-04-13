#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let channel = {
        let amqp_url = std::env::var("AMQP_URL")?;
        let options = lapin::ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio);
        let conn = lapin::Connection::connect(&amqp_url, options).await?;
        conn.create_channel().await?
    };

    let pool = {
        let database_url = std::env::var("DATABASE_URL")?;
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await?
    };

    let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;

    listener.listen("outbox_channel").await?;

    let listener_stream = {
        let stream = listener.into_stream();
        futures_util::StreamExt::map(stream, |r| r.map(|_| ()))
    };

    let timer_stream = {
        let duration = tokio::time::Duration::from_secs(5);
        let interval = tokio::time::interval(duration);
        let stream = tokio_stream::wrappers::IntervalStream::new(interval);
        futures_util::StreamExt::map(stream, |_| Ok(()))
    };

    let mut combined_stream = tokio_stream::StreamExt::merge(listener_stream, timer_stream);

    while let Ok(Some(_)) = futures_util::TryStreamExt::try_next(&mut combined_stream).await {
        let _ = process_outbox(&pool, async |outbox| {
            println!("Processing outbox message: {:?}", outbox);

            channel
                .basic_publish(
                    "",
                    &outbox.topic,
                    lapin::options::BasicPublishOptions::default(),
                    outbox.payload.as_bytes(),
                    lapin::BasicProperties::default(),
                )
                .await?;

            Ok(())
        })
        .await;
    }

    Ok(())
}

#[derive(Debug)]
struct OutboxMessage {
    id: i64,
    topic: String,
    payload: String,
}

async fn process_outbox(
    pool: &sqlx::Pool<sqlx::Postgres>,
    handler: impl AsyncFn(&OutboxMessage) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tx = pool.begin().await?;

    let list = sqlx::query_as!(
        OutboxMessage,
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
        handler(&outbox).await?;

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
