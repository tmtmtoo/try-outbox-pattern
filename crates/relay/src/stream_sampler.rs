pub struct TimedSampler<S>
where
    S: tokio_stream::Stream + Unpin,
{
    stream: S,
    last_emit: tokio::time::Instant,
    interval: tokio::time::Duration,
    buffer: Option<S::Item>,
}

impl<S> TimedSampler<S>
where
    S: tokio_stream::Stream + Unpin,
{
    fn new(stream: S, interval: tokio::time::Duration) -> Self {
        Self {
            stream,
            last_emit: tokio::time::Instant::now(),
            interval,
            buffer: None,
        }
    }
}

impl<S> tokio_stream::Stream for TimedSampler<S>
where
    S: tokio_stream::Stream + Unpin,
    <S as tokio_stream::Stream>::Item: Unpin,
{
    type Item = S::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            match std::pin::Pin::new(&mut this.stream).poll_next(cx) {
                std::task::Poll::Ready(Some(item)) => {
                    let now = tokio::time::Instant::now();
                    if now.duration_since(this.last_emit) >= this.interval {
                        this.last_emit = now;
                        return std::task::Poll::Ready(Some(item));
                    } else {
                        this.buffer = Some(item);
                        continue;
                    }
                }
                std::task::Poll::Ready(None) => {
                    if let Some(item) = this.buffer.take() {
                        return std::task::Poll::Ready(Some(item));
                    } else {
                        return std::task::Poll::Ready(None);
                    }
                }
                std::task::Poll::Pending => match &this.buffer {
                    Some(_) => {
                        let now = tokio::time::Instant::now();
                        if now.duration_since(this.last_emit) >= this.interval {
                            this.last_emit = now;
                            return std::task::Poll::Ready(this.buffer.take());
                        } else {
                            return std::task::Poll::Pending;
                        }
                    }
                    None => {
                        return std::task::Poll::Pending;
                    }
                },
            }
        }
    }
}

pub trait TimedSampling {
    fn sampling(self, interval: tokio::time::Duration) -> TimedSampler<Self>
    where
        Self: Sized + Unpin,
        Self: tokio_stream::Stream,
    {
        TimedSampler::new(self, interval)
    }
}

impl<S> TimedSampling for S where S: tokio_stream::Stream + Unpin {}
