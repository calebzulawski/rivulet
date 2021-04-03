use rand::{rngs::SmallRng, Rng, SeedableRng};
use rivulet::{
    buffer::circular_buffer::{spmc, spsc},
    Error, Stream, StreamMut,
};
use std::hash::Hasher;
use tokio::sync::oneshot;

static BUFFER_SIZE: usize = 4096;

async fn write<T: StreamMut<Item = i64> + Send + Unpin>(
    mut sink: T,
    block: usize,
    count: usize,
    sender: oneshot::Sender<u64>,
) {
    let mut hasher = seahash::SeaHasher::new();
    let mut rng = SmallRng::from_entropy();
    for _ in 0..count {
        sink.grant(block).await.unwrap();
        for value in &mut sink.stream_mut()[..block] {
            *value = rng.gen();
            hasher.write_i64(*value);
        }
        sink.release(block).await.unwrap();
    }
    sender.send(hasher.finish()).unwrap();
}

async fn read<T: Stream<Item = i64> + Send + Unpin>(mut source: T, sender: oneshot::Sender<u64>) {
    let mut hasher = seahash::SeaHasher::new();
    let mut rng = SmallRng::from_entropy();
    let mut closed = false;
    while !closed {
        let count = rng.gen_range(1, BUFFER_SIZE);
        match source.grant(count).await {
            Err(Error::Closed) => {
                closed = true;
                assert!(source.stream().len() <= count);
            }
            x => {
                x.unwrap();
                assert!(source.stream().len() >= count);
            }
        }
        for value in source.stream() {
            hasher.write_i64(*value);
        }
        let released = source.stream().len();
        source.release(released).await.unwrap();
    }
    sender.send(hasher.finish()).unwrap();
}

#[tokio::test]
async fn spsc_buffer_integrity() {
    let (sink, source) = spsc::buffer::<i64>(BUFFER_SIZE);

    let (write_send, write_recv) = oneshot::channel::<u64>();
    let (read_send, read_recv) = oneshot::channel::<u64>();

    tokio::spawn(write(sink, 500, 400, write_send));
    tokio::spawn(read(source, read_send));

    let (write_hash, read_hash) = futures::future::join(write_recv, read_recv).await;
    assert_eq!(write_hash.unwrap(), read_hash.unwrap());
}

#[tokio::test]
async fn spmc_buffer_integrity() {
    let (sink, source) = spmc::buffer::<i64>(BUFFER_SIZE);

    let (write_send, write_recv) = oneshot::channel::<u64>();
    tokio::spawn(write(sink, 500, 400, write_send));

    let mut read_recv = Vec::new();
    for _ in 0..10 {
        let (send, recv) = oneshot::channel::<u64>();
        tokio::spawn(read(source.clone(), send));
        read_recv.push(recv);
    }
    std::mem::drop(source); // remaining reference doesn't get used, so drop it

    let (write_hash, read_hashes) =
        futures::future::join(write_recv, futures::future::join_all(read_recv)).await;
    for read_hash in read_hashes {
        assert_eq!(write_hash.as_ref().unwrap(), read_hash.as_ref().unwrap());
    }
}
