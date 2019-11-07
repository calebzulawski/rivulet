use rivulet::buffer::spsc;
use rivulet::stream::{Sink, Source};
use rand::Rng;
use std::hash::Hasher;

static BUFFER_SIZE: usize = 4096;

#[tokio::test]
async fn spsc_buffer_integrity() {
    async fn write(mut sink: spsc::BufferSink<i64>, block: usize, count: usize) -> (usize, u64) {
        let mut hasher = seahash::SeaHasher::new();
        let mut rng = rand::thread_rng();
        let mut total = 0;
        for _ in 0..count {
            sink = sink.next(block).await.unwrap();
            for value in sink.iter_mut() {
                *value = rng.gen();
                hasher.write_i64(*value);
            }
            total += block;
        }
        sink.advance(block, 0).await.unwrap();
        return (total, hasher.finish());
    }
    
    async fn read(mut source: spsc::BufferSource<i64>) -> (usize, u64) {
        let mut hasher = seahash::SeaHasher::new();
        let mut rng = rand::thread_rng();
        let mut total = 0;
        loop {
            let block = rng.gen_range(1, BUFFER_SIZE);
            source = match source.next(block).await {
                Some(source) => {
                    for value in source.iter() {
                        hasher.write_i64(*value);
                    }
                    total += source.len();
                    source
                },
                None => return (total, hasher.finish())
            };
        }
    }

    let (sink, source) = spsc::buffer::<i64>(BUFFER_SIZE);
    let (write_hash, read_hash) = futures::future::join(write(sink, 500, 400), read(source)).await;
    assert_eq!(write_hash, read_hash);
}
