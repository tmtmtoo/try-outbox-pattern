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
        let _ = process_outbox(&pool, 2).await;
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
    pool: &sqlx::Pool<sqlx::Postgres>,
    limit: u32,
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
        limit
            $1
        for update
            skip locked
    "#,
        i64::from(limit)
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
