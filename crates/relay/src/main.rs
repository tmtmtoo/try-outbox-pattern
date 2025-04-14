#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let amqp_channel = {
        let amqp_url = std::env::var("AMQP_URL")?;
        let options = lapin::ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio);
        let conn = lapin::Connection::connect(&amqp_url, options).await?;
        conn.create_channel().await?
    };

    let pg_pool = {
        let database_url = std::env::var("DATABASE_URL")?;
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await?
    };

    let pg_listener_stream = {
        let mut listener = sqlx::postgres::PgListener::connect_with(&pg_pool).await?;
        listener.listen("outbox_channel").await?;
        let stream = listener.into_stream();

        use futures_util::stream::StreamExt;
        stream.map(|r| r.map(|_| ()))
    };

    let timer_stream = {
        let duration = tokio::time::Duration::from_secs(5);
        let interval = tokio::time::interval(duration);
        let stream = tokio_stream::wrappers::IntervalStream::new(interval);

        use futures_util::stream::StreamExt;
        stream.map(|_| Ok(()))
    };

    let mut outbox_handle_stream = {
        use tokio_stream::StreamExt;
        pg_listener_stream.merge(timer_stream)
    };

    use futures_util::TryStreamExt;
    while let Ok(Some(_)) = outbox_handle_stream.try_next().await {
        process_outbox(&pg_pool, async |outbox| {
            println!("Processing outbox message: {:?}", outbox);

            amqp_channel
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
        .await?;
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
