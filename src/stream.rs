use async_trait::async_trait;

#[async_trait]
pub trait Sink<T: Send + Sync + 'static>: Sized + Sync {
    fn as_mut_slice(&mut self) -> &mut [T];

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    async fn advance(self, advance: usize, size: usize) -> Option<Self>;

    async fn next(self, size: usize) -> Option<Self> {
        let advance = self.len();
        self.advance(advance, size).await
    }

    async fn resize(self, size: usize) -> Option<Self> {
        self.advance(0, size).await
    }
}

#[async_trait]
pub trait Source<T: Send + Sync + 'static>: Sized + Sync {
    fn as_slice(&self) -> &[T];

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    async fn advance(self, advance: usize, size: usize) -> Option<Self>;

    async fn next(self, size: usize) -> Option<Self> {
        let advance = self.len();
        self.advance(advance, size).await
    }

    async fn resize(self, size: usize) -> Option<Self> {
        self.advance(0, size).await
    }
}
