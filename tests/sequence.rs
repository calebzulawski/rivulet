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

async fn process<T: ViewMut<Item = i64> + Send>(mut view: T) {
    let mut rng = SmallRng::from_entropy();
    loop {
        let count = rng.gen_range(1..BUFFER_SIZE / 2);
        view.grant(count).await.unwrap();
        if view.view().is_empty() {
            break;
        }
        for value in view.view_mut() {
            *value *= -1;
        }
        view.release(view.view().len());
    }
}

async fn read<T: View<Item = i64> + Send>(mut source: T, processed: bool) -> u64 {
    let mut hasher = seahash::SeaHasher::new();
    let mut rng = SmallRng::from_entropy();
    let factor = if processed { -1 } else { 1 };
    loop {
        let count = rng.gen_range(1..BUFFER_SIZE / 2);
        source.grant(count).await.unwrap();
        if source.view().is_empty() {
            break hasher.finish();
        }
        for value in source.view() {
            hasher.write_i64(factor * *value);
        }
        source.release(source.view().len());
    }
}

#[tokio::test]
async fn sequence_single_reader() {
    let (sink, source) = circular_buffer::<i64>(BUFFER_SIZE);
    let (first, second) = source.sequence();

    let write_hash = tokio::spawn(write(sink, 500, 400));
    let process = tokio::spawn(process(first.into_view()));
    let read_hash = tokio::spawn(read(second.into_view(), true));

    let (write_hash, read_hash, process) = tokio::join!(write_hash, read_hash, process);

    process.unwrap();
    assert_eq!(write_hash.unwrap(), read_hash.unwrap());
}

#[tokio::test]
async fn sequence_multiple_reader() {
    let (sink, source) = circular_buffer::<i64>(BUFFER_SIZE);
    let (first, second) = source.sequence();

    let second = second.into_cloneable_view();

    let write_hash = tokio::spawn(write(sink, 500, 400));
    let process = tokio::spawn(process(first.into_view()));
    let read_hashes = (0..10)
        .map(|_| tokio::spawn(read(second.clone(), true)))
        .collect::<Vec<_>>();
    std::mem::drop(second); // remaining reference doesn't get used, so drop it

    let (write_hash, read_hashes, process) =
        tokio::join!(write_hash, futures::future::join_all(read_hashes), process);

    process.unwrap();
    for read_hash in read_hashes {
        assert_eq!(write_hash.as_ref().unwrap(), read_hash.as_ref().unwrap());
    }
}

#[tokio::test]
async fn sequence_drop_first() {
    let (mut sink, source) = circular_buffer::<i64>(BUFFER_SIZE);
    let (_, second) = source.sequence();
    let mut second = second.into_view();

    // Add a bit of data.
    sink.grant(1).await.unwrap();
    sink.release(1);

    // The stream is closed, so this should return an empty view.
    second.grant(100).await.unwrap();
    assert_eq!(second.view(), &[]);
}

#[tokio::test]
async fn sequence_drop_second() {
    let (sink, source) = circular_buffer::<i64>(BUFFER_SIZE);
    let (first, _) = source.sequence();

    let write_hash = tokio::spawn(write(sink, 500, 400));
    let read_hash = tokio::spawn(read(first.into_view(), false));

    let (write_hash, read_hash) = tokio::join!(write_hash, read_hash);

    assert_eq!(write_hash.unwrap(), read_hash.unwrap());
}
