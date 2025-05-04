#[derive(Debug, serde::Deserialize)]
struct Config {
    amqp_url: String,
    database_url: String,
    relay_throttle_millis: u32,
    relay_query_limit: Option<u32>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = envy::from_env::<Config>()?;

    let amqp_channel = {
        let options = lapin::ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio);
        let conn = lapin::Connection::connect(&config.amqp_url, options).await?;
        conn.create_channel().await?
    };

    let pg_pool = {
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&config.database_url)
            .await?
    };

    process_outbox(&pg_pool, config.relay_query_limit, async |message| {
        amqp_channel
            .basic_publish(
                "",
                &message.topic,
                lapin::options::BasicPublishOptions::default(),
                message.payload.as_bytes(),
                lapin::BasicProperties::default(),
            )
            .await
            .map(|_| ())
            .map_err(Into::into)
    })
    .await?;

    let pg_listener_stream = {
        let mut listener = sqlx::postgres::PgListener::connect_with(&pg_pool).await?;
        listener.listen("outbox_channel").await?;
        let stream = listener.into_stream();

        use tokio_stream::StreamExt;
        stream.throttle(tokio::time::Duration::from_millis(
            config.relay_throttle_millis.into(),
        ))
    };
    tokio::pin!(pg_listener_stream);

    use futures_util::TryStreamExt;
    while let Ok(Some(_)) = pg_listener_stream.try_next().await {
        process_outbox(&pg_pool, config.relay_query_limit, async |message| {
            amqp_channel
                .basic_publish(
                    "",
                    &message.topic,
                    lapin::options::BasicPublishOptions::default(),
                    message.payload.as_bytes(),
                    lapin::BasicProperties::default(),
                )
                .await
                .map(|_| ())
                .map_err(Into::into)
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
    query_limit: Option<u32>,
    handler: impl AsyncFn(&OutboxMessage) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let mut tx = pool.begin().await?;

        let messages = match query_limit {
            Some(limit) => {
                sqlx::query_as!(
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
                limit
                    $1
                for update
                    skip locked
            "#,
                    limit as i64
                )
                .fetch_all(&mut *tx)
                .await?
            }
            None => {
                sqlx::query_as!(
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
                .await?
            }
        };

        let mut processed_outbox_ids = Vec::with_capacity(messages.len());

        for message in &messages {
            if let Ok(_) = handler(message).await {
                processed_outbox_ids.push(message.id);
            }
        }

        if !processed_outbox_ids.is_empty() {
            sqlx::query!(
                r#"
                update outbox
                set processed_at = now()
                where id = any($1)
            "#,
                &processed_outbox_ids
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        match (query_limit, messages.len()) {
            (Some(limit), len) if len < limit as usize => break,
            (None, _) => break,
            _ => continue,
        }
    }

    Ok(())
}
