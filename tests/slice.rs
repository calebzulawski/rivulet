use rivulet::{
    slice::{Slice, SliceMut},
    SplittableView, View, ViewMut,
};

#[test]
fn slice() {
    let storage: Vec<u8> = (0..100).collect();
    let mut stream = Slice::new(&storage).into_view();

    // There should be an immediate view available
    stream.blocking_grant(0).unwrap();
    let view = stream.view();
    assert_eq!(view.len(), 100);
    assert_eq!(view[0], 0);

    // Request a grant, nothing should change
    stream.blocking_grant(10).unwrap();
    let view = stream.view();
    assert_eq!(view.len(), 100);
    assert_eq!(view[0], 0);

    // Release a few items and change a value
    stream.release(5);
    let view = stream.view();
    assert_eq!(view.len(), 95);
    assert_eq!(view[0], 5);

    // Do a giant (but useless) grant request and release the remaining items except one
    stream.blocking_grant(1000).unwrap();
    stream.release(94);
    let view = stream.view();
    assert_eq!(view.len(), 1);
    assert_eq!(view[0], 99);

    // Release the last item
    stream.release(1);
    let view = stream.view();
    assert!(view.is_empty());
}

#[test]
fn slice_mut() {
    let mut storage: Vec<u8> = (0..100).collect();
    let mut stream = SliceMut::new(&mut storage).into_view();

    // There should be an immediate view available
    stream.blocking_grant(0).unwrap();
    let view = stream.view();
    assert_eq!(view.len(), 100);
    assert_eq!(view[0], 0);

    // Request a grant, nothing should change
    stream.blocking_grant(10).unwrap();
    let view = stream.view();
    assert_eq!(view.len(), 100);
    assert_eq!(view[0], 0);

    // Release a few items and change a value
    stream.release(5);
    let view = stream.view_mut();
    assert_eq!(view.len(), 95);
    assert_eq!(view[0], 5);
    view[0] = 101;

    // Do a giant (but useless) grant request and release the remaining items except one
    stream.blocking_grant(1000).unwrap();
    stream.release(94);
    let view = stream.view();
    assert_eq!(view.len(), 1);
    assert_eq!(view[0], 99);

    // Release the last item
    stream.release(1);
    let view = stream.view();
    assert!(view.is_empty());

    // Check that the storage contains the original with the modification
    let expected = {
        let mut expected: Vec<u8> = (0..100).collect();
        expected[5] = 101;
        expected
    };
    assert_eq!(storage, expected);
}

#[test]
#[should_panic]
fn bad_release() {
    let storage: Vec<u8> = (0..100).collect();
    let mut stream = Slice::new(&storage).into_view();
    stream.release(101);
}
