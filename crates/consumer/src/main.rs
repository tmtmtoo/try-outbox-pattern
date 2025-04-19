#[derive(Debug, serde::Deserialize)]
struct Config {
    amqp_url: String,
    consumer_duration_secs: u32,
    consumer_failure_rate: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use futures_util::stream::TryStreamExt;

    let config = envy::from_env::<Config>()?;

    let amqp_channel = {
        let options = lapin::ConnectionProperties::default()
            .with_executor(tokio_executor_trait::Tokio::current())
            .with_reactor(tokio_reactor_trait::Tokio);
        let conn = lapin::Connection::connect(&config.amqp_url, options).await?;
        conn.create_channel().await.map(std::sync::Arc::new)?
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

    while let Some(message) = stream.try_next().await? {
        let amqp_channel = amqp_channel.clone();
        let task = async move {
            process_message(amqp_channel.clone(), message, async move |event| {
                println!("Received event: {:?}", event);
                tokio::time::sleep(tokio::time::Duration::from_secs(
                    config.consumer_duration_secs.into(),
                ))
                .await;
                random_error(config.consumer_failure_rate)?;
                Ok(())
            })
            .await
        };
        tokio::spawn(task);
    }

    Ok(())
}

async fn process_message(
    amqp_channel: std::sync::Arc<lapin::Channel>,
    message: lapin::message::Delivery,
    handler: impl AsyncFn(
        event::Posted,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>>,
) {
    let data = String::from_utf8_lossy(&message.data);

    let event = match serde_json::from_str::<event::Posted>(&data) {
        Ok(event) => event,
        Err(_) => {
            let _ = amqp_channel
                .basic_nack(
                    message.delivery_tag,
                    lapin::options::BasicNackOptions {
                        requeue: true,
                        ..Default::default()
                    },
                )
                .await;
            return;
        }
    };

    match handler(event).await {
        Ok(_) => {
            let _ = amqp_channel
                .basic_ack(
                    message.delivery_tag,
                    lapin::options::BasicAckOptions::default(),
                )
                .await;
        }
        Err(_) => {
            let _ = amqp_channel
                .basic_nack(
                    message.delivery_tag,
                    lapin::options::BasicNackOptions {
                        requeue: true,
                        ..Default::default()
                    },
                )
                .await;
        }
    }
}

fn random_error(
    failure_rate: f64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    if rand::random_range(0.0..=1.0) < failure_rate {
        Err("Random error occurred".into())
    } else {
        Ok(())
    }
}
