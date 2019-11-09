use rand::{rngs::SmallRng, Rng, SeedableRng};
use rivulet::buffer::spsc;
use rivulet::stream::{Sink, Source};
use std::hash::Hasher;
use tokio::sync::oneshot;

static BUFFER_SIZE: usize = 4096;

#[tokio::test]
async fn spsc_buffer_integrity() {
    async fn write(
        mut sink: spsc::BufferSink<i64>,
        block: usize,
        count: usize,
        sender: oneshot::Sender<u64>,
    ) {
        let mut hasher = seahash::SeaHasher::new();
        let mut rng = SmallRng::from_entropy();
        for _ in 0..count {
            let buffer = sink.next(block).await.unwrap();
            for value in buffer {
                *value = rng.gen();
                hasher.write_i64(*value);
            }
        }
        sink.advance(block, 0).await.unwrap();
        sender.send(hasher.finish()).unwrap();
    }

    async fn read(mut source: spsc::BufferSource<i64>, sender: oneshot::Sender<u64>) {
        let mut hasher = seahash::SeaHasher::new();
        let mut rng = SmallRng::from_entropy();
        while let Some(buffer) = source.next(rng.gen_range(1, BUFFER_SIZE)).await {
            for value in buffer {
                hasher.write_i64(*value);
            }
        }
        sender.send(hasher.finish()).unwrap();
    }

    let (sink, source) = spsc::buffer::<i64>(BUFFER_SIZE);

    let (write_send, write_recv) = oneshot::channel::<u64>();
    let (read_send, read_recv) = oneshot::channel::<u64>();

    tokio::spawn(write(sink, 500, 400, write_send));
    tokio::spawn(read(source, read_send));

    let (write_hash, read_hash) = futures::future::join(write_recv, read_recv).await;
    assert_eq!(write_hash.unwrap(), read_hash.unwrap());
}
