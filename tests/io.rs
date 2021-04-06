use rand::{rngs::SmallRng, Rng, SeedableRng};
use rivulet::buffer::circular_buffer::spsc::buffer;
use std::hash::Hash;

#[tokio::test]
async fn async_reader_writer() {
    use futures::io::{AsyncReadExt, AsyncWriteExt};

    let (sink, source) = buffer(4096);
    let mut write = rivulet::io::AsyncWriter::new(sink);
    let mut read = rivulet::io::AsyncReader::new(source);

    let sent = tokio::spawn(async move {
        let mut rng = SmallRng::from_entropy();
        let values: Vec<u8> = (0..1_000_000).map(|_| rng.gen()).collect();
        write.write_all(&values).await.unwrap();
        values.hash(&mut seahash::SeaHasher::new());
    });

    let received = tokio::spawn(async move {
        let mut values = Vec::new();
        read.read_to_end(&mut values).await.unwrap();
        values.hash(&mut seahash::SeaHasher::new())
    });

    let (sent, received) = futures::future::join(sent, received).await;
    assert_eq!(sent.unwrap(), received.unwrap());
}

#[test]
fn sync_reader_writer() {
    use std::io::{Read, Write};

    let (sink, source) = buffer(4096);
    let mut write = rivulet::io::Writer::new(sink);
    let mut read = rivulet::io::Reader::new(source);

    let sent = std::thread::spawn(move || {
        let mut rng = SmallRng::from_entropy();
        let values: Vec<u8> = (0..1_000_000).map(|_| rng.gen()).collect();
        write.write_all(&values).unwrap();
        values.hash(&mut seahash::SeaHasher::new());
    });

    let received = std::thread::spawn(move || {
        let mut values = Vec::new();
        read.read_to_end(&mut values).unwrap();
        values.hash(&mut seahash::SeaHasher::new())
    });

    let sent = sent.join().unwrap();
    let received = received.join().unwrap();
    assert_eq!(sent, received);
}
