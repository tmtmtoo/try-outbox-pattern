pub struct TimedSampler<S>
where
    S: tokio_stream::Stream + Unpin,
{
    stream: S,
    last_emit: tokio::time::Instant,
    interval: tokio::time::Duration,
    buffer: Option<S::Item>,
    wake_timer: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
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
            wake_timer: None,
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
                        this.buffer = None;
                        this.wake_timer = None;
                        return std::task::Poll::Ready(Some(item));
                    } else {
                        this.buffer = Some(item);
                        if this.wake_timer.is_none() {
                            let remain = this
                                .interval
                                .saturating_sub(now.duration_since(this.last_emit));
                            this.wake_timer = Some(Box::pin(tokio::time::sleep(remain)));
                        }
                        continue;
                    }
                }
                std::task::Poll::Ready(None) => {
                    if let Some(item) = this.buffer.take() {
                        this.wake_timer = None;
                        return std::task::Poll::Ready(Some(item));
                    } else {
                        return std::task::Poll::Ready(None);
                    }
                }
                std::task::Poll::Pending => match (&this.buffer, &mut this.wake_timer) {
                    (Some(_), Some(wake_timer)) => match wake_timer.as_mut().poll(cx) {
                        std::task::Poll::Ready(()) => {
                            this.wake_timer = None;
                            this.last_emit = tokio::time::Instant::now();
                            return std::task::Poll::Ready(this.buffer.take());
                        }
                        std::task::Poll::Pending => {
                            return std::task::Poll::Pending;
                        }
                    },
                    (Some(_), None) => {
                        let now = tokio::time::Instant::now();
                        let deadline = now + (this.interval - now.duration_since(this.last_emit));
                        this.wake_timer = Some(Box::pin(tokio::time::sleep_until(deadline)));
                        return std::task::Poll::Pending;
                    }
                    _ => return std::task::Poll::Pending,
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
