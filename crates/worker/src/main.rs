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

    let mut stream = amqp_channel
        .basic_consume(
            "post",
            "post",
            lapin::options::BasicConsumeOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;

    use futures_util::stream::TryStreamExt;
    while let Some(message) = stream.try_next().await? {
        println!("Received message: {}", String::from_utf8_lossy(&message.data));
    }

    Ok(())
}
