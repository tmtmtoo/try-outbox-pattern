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
            event::topic::POST,
            env!("CARGO_PKG_NAME"),
            lapin::options::BasicConsumeOptions {
                no_ack: false,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    use futures_util::stream::TryStreamExt;
    while let Some(message) = stream.try_next().await? {
        let data = String::from_utf8_lossy(&message.data);
        let posted = serde_json::from_str::<event::Posted>(&data)?;
        println!("Received message: {posted:?}");

        let timeout_duration = tokio::time::Duration::from_secs(5);
        let result = tokio::time::timeout(
            timeout_duration,
            process_may_error(rand::random_range(0..7)),
        )
        .await;

        match result {
            Ok(Ok(_)) => {
                println!("Processing succeeded");
                amqp_channel
                    .basic_ack(
                        message.delivery_tag,
                        lapin::options::BasicAckOptions::default(),
                    )
                    .await?;
            }
            Ok(Err(_)) => {
                println!("Processing failed, requeuing");
                amqp_channel
                    .basic_nack(
                        message.delivery_tag,
                        lapin::options::BasicNackOptions {
                            requeue: true,
                            ..Default::default()
                        },
                    )
                    .await?;
            }
            Err(_) => {
                println!("Processing timed out, requeuing");
                amqp_channel
                    .basic_nack(
                        message.delivery_tag,
                        lapin::options::BasicNackOptions {
                            requeue: true,
                            ..Default::default()
                        },
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

async fn process_may_error(duration_sec: u32) -> Result<(), ()> {
    tokio::time::sleep(tokio::time::Duration::from_secs(duration_sec.into())).await;
    if rand::random::<bool>() {
        Ok(())
    } else {
        Err(())
    }
}
