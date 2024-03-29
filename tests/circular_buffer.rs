use rand::{rngs::SmallRng, Rng, SeedableRng};
use rivulet::{circular_buffer, SplittableView, View, ViewMut};
use std::hash::Hasher;

static BUFFER_SIZE: usize = 4096;

async fn write<T: ViewMut<Item = i64> + Send>(mut sink: T, block: usize, count: usize) -> u64 {
    let mut hasher = seahash::SeaHasher::new();
    let mut rng = SmallRng::from_entropy();
    for _ in 0..count {
        sink.grant(block).await.unwrap();
        for value in &mut sink.view_mut()[..block] {
            *value = rng.gen();
            hasher.write_i64(*value);
        }
        sink.release(block);
    }
    hasher.finish()
}

async fn read<T: View<Item = i64> + Send>(mut source: T) -> u64 {
    let mut hasher = seahash::SeaHasher::new();
    let mut rng = SmallRng::from_entropy();
    loop {
        let count = rng.gen_range(1..BUFFER_SIZE);
        source.grant(count).await.unwrap();
        if source.view().is_empty() {
            break hasher.finish();
        }
        for value in source.view() {
            hasher.write_i64(*value);
        }
        let released = source.view().len();
        source.release(released);
    }
}

#[tokio::test]
async fn single_reader_buffer_integrity() {
    let (sink, source) = circular_buffer::<i64>(BUFFER_SIZE);

    let write_hash = tokio::spawn(write(sink, 500, 400));
    let read_hash = tokio::spawn(read(source.into_view()));

    let (write_hash, read_hash) = futures::future::join(write_hash, read_hash).await;
    assert_eq!(write_hash.unwrap(), read_hash.unwrap());
}

#[tokio::test]
async fn multiple_reader_buffer_integrity() {
    let (sink, source) = circular_buffer::<i64>(BUFFER_SIZE);
    let source = source.into_cloneable_view();

    let write_hash = tokio::spawn(write(sink, 500, 400));
    let read_hashes = (0..10)
        .map(|_| tokio::spawn(read(source.clone())))
        .collect::<Vec<_>>();
    std::mem::drop(source); // remaining reference doesn't get used, so drop it

    let (write_hash, read_hashes) =
        futures::future::join(write_hash, futures::future::join_all(read_hashes)).await;
    for read_hash in read_hashes {
        assert_eq!(write_hash.as_ref().unwrap(), read_hash.as_ref().unwrap());
    }
}
