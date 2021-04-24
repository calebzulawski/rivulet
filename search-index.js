var searchIndex = JSON.parse('{\
"rivulet":{"doc":"Rivulet provides tools for creating and processing …","t":[3,16,16,3,16,8,8,8,8,11,11,11,11,11,11,11,11,0,0,11,11,11,11,11,11,11,11,0,11,11,10,10,11,11,11,11,11,11,11,11,11,11,10,10,0,0,0,3,3,11,11,11,11,5,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,3,3,11,11,11,11,5,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,12,3,4,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,3,3,3,3,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11,11],"n":["Grant","GrantError","Item","Release","ReleaseError","Sink","Source","View","ViewMut","blocking_grant","blocking_grant","blocking_release","blocking_release","borrow","borrow","borrow_mut","borrow_mut","buffer","error","from","from","grant","grant","into","into","into_future","into_future","io","poll","poll","poll_grant","poll_release","release","release","try_from","try_from","try_into","try_into","try_poll","try_poll","type_id","type_id","view","view_mut","circular_buffer","spmc","spsc","Sink","Source","borrow","borrow","borrow_mut","borrow_mut","buffer","clone","clone_into","from","from","into","into","poll_grant","poll_grant","poll_release","poll_release","to_owned","try_from","try_from","try_into","try_into","type_id","type_id","view","view","view_mut","Sink","Source","borrow","borrow","borrow_mut","borrow_mut","buffer","from","from","into","into","poll_grant","poll_grant","poll_release","poll_release","try_from","try_from","try_into","try_into","type_id","type_id","view","view","view_mut","view_mut","0","GrantOverflow","Infallible","borrow","borrow","borrow_mut","borrow_mut","clone","clone","clone_into","clone_into","fmt","fmt","fmt","fmt","from","from","from","into","into","source","source","to_owned","to_owned","to_string","to_string","try_from","try_from","try_into","try_into","type_id","type_id","AsyncReader","AsyncWriter","Reader","Writer","borrow","borrow","borrow","borrow","borrow_mut","borrow_mut","borrow_mut","borrow_mut","clone","clone","clone","clone","clone_into","clone_into","clone_into","clone_into","consume","consume","fill_buf","flush","fmt","fmt","fmt","fmt","from","from","from","from","into","into","into","into","into_inner","into_inner","into_inner","into_inner","new","new","new","new","poll_close","poll_fill_buf","poll_flush","poll_read","poll_write","read","to_owned","to_owned","to_owned","to_owned","try_from","try_from","try_from","try_from","try_into","try_into","try_into","try_into","type_id","type_id","type_id","type_id","write"],"q":["rivulet","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","rivulet::buffer","rivulet::buffer::circular_buffer","","rivulet::buffer::circular_buffer::spmc","","","","","","","","","","","","","","","","","","","","","","","","","","","rivulet::buffer::circular_buffer::spsc","","","","","","","","","","","","","","","","","","","","","","","","","rivulet::error","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","rivulet::io","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","",""],"d":["Future produced by [<code>View::grant</code>].","The error produced by <code>poll_grant</code>.","The streamed type.","Future produced by [<code>View::release</code>].","The error produced by <code>poll_release</code>.","A marker trait that indicates this view is a sink of data.","A marker trait that indicates this view is a source of …","Obtain views into asynchronous contiguous-memory streams.","Obtain mutable views into asynchronous contiguous-memory …","Obtains a view of at least <code>count</code> elements, blocking the …","Obtains a view of at least <code>count</code> elements, blocking the …","Advances past the first <code>count</code> elements in the current …","Advances past the first <code>count</code> elements in the current …","","","","","Asynchronous buffers for temporarily caching data.","Errors produced by streams.","","","Create a future that obtains a view of at least <code>count</code> …","Create a future that obtains a view of at least <code>count</code> …","","","","","Utilities for working with [<code>std::io</code>].","","","Attempt to obtain a view of at least <code>count</code> elements.","Attempt to advance past the first <code>count</code> elements in the …","Create a future that advances past the first <code>count</code> …","Create a future that advances past the first <code>count</code> …","","","","","","","","","Obtain the current view of the stream.","Obtain the current mutable view of the stream.","Async circular buffers.","A single-producer, multiple-releaser async circular …","A single-producer, single-consumer async circular buffer.","Write values to the associated <code>Source</code>s.","Read values from the associated <code>Sink</code>.","","","","","Creates a single-producer, multiple-releaser async …","","","","","","","","","","","","","","","","","","","","","Write values to the associated <code>Source</code>.","Read values from the associated <code>Sink</code>.","","","","","Creates a single-producer, single-consumer async circular …","","","","","","","","","","","","","","","","","","","","Error produced when a request is too large to grant.","Error used when a grant or release will never fail.","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","Implements <code>futures::io::AsyncRead</code> for a source.","Implements <code>futures::io::AsyncWrite</code> for a sink.","Implements [<code>std::io::Read</code>] for a source.","Implements [<code>std::io::Write</code>] for a sink.","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","","Return the original <code>Source</code>","Return the original <code>Source</code>","Return the original <code>Sink</code>","Return the original <code>Sink</code>","Create a new <code>Reader</code>","Create a new <code>AsyncReader</code>","Create a new <code>Writer</code>","Create a new <code>AsyncWriter</code>","","","","","","","","","","","","","","","","","","","","","","",""],"i":[0,1,1,0,1,0,0,0,0,1,1,1,1,2,3,2,3,0,0,2,3,1,1,2,3,2,3,0,2,3,1,1,1,1,2,3,2,3,2,3,2,3,1,4,0,0,0,0,0,5,6,5,6,0,6,6,5,6,5,6,5,6,5,6,6,5,6,5,6,5,6,5,6,5,0,0,7,8,7,8,0,7,8,7,8,7,8,7,8,7,8,7,8,7,8,7,8,7,8,9,0,0,10,9,10,9,10,9,10,9,10,10,9,9,10,9,9,10,9,10,9,10,9,10,9,10,9,10,9,10,9,0,0,0,0,11,12,13,14,11,12,13,14,11,12,13,14,11,12,13,14,11,12,11,13,11,12,13,14,11,12,13,14,11,12,13,14,11,12,13,14,11,12,13,14,14,12,14,12,14,11,11,12,13,14,11,12,13,14,11,12,13,14,11,12,13,14,13],"f":[null,null,null,null,null,null,null,null,null,[[["usize",15]],["result",4]],[[["usize",15]],["result",4]],[[["usize",15]],["result",4]],[[["usize",15]],["result",4]],[[]],[[]],[[]],[[]],null,null,[[]],[[]],[[["usize",15]],["grant",3]],[[["usize",15]],["grant",3]],[[]],[[]],[[]],[[]],null,[[["context",3],["pin",3]],["poll",4]],[[["context",3],["pin",3]],["poll",4]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["usize",15]],["release",3]],[[["usize",15]],["release",3]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[["pin",3],["context",3]],["poll",4]],[[["pin",3],["context",3]],["poll",4]],[[],["typeid",3]],[[],["typeid",3]],[[]],[[]],null,null,null,null,null,[[]],[[]],[[]],[[]],[[["usize",15]]],[[],["source",3]],[[]],[[]],[[]],[[]],[[]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["typeid",3]],[[],["typeid",3]],[[]],[[]],[[]],null,null,[[]],[[]],[[]],[[]],[[["usize",15]]],[[]],[[]],[[]],[[]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[["context",3],["usize",15],["pin",3]],[["poll",4],["result",4]]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["typeid",3]],[[],["typeid",3]],[[]],[[]],[[]],[[]],null,null,null,[[]],[[]],[[]],[[]],[[],["infallible",4]],[[],["grantoverflow",3]],[[]],[[]],[[["formatter",3]],["result",6]],[[["formatter",3]],["result",6]],[[["formatter",3]],["result",6]],[[["formatter",3]],[["result",4],["error",3]]],[[]],[[]],[[["infallible",4]]],[[]],[[]],[[],[["option",4],["error",8]]],[[],[["option",4],["error",8]]],[[]],[[]],[[],["string",3]],[[],["string",3]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["typeid",3]],[[],["typeid",3]],null,null,null,null,[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[],["reader",3]],[[],["asyncreader",3]],[[],["writer",3]],[[],["asyncwriter",3]],[[]],[[]],[[]],[[]],[[["usize",15]]],[[["usize",15],["pin",3]]],[[],["result",6]],[[],["result",6]],[[["formatter",3]],["result",6]],[[["formatter",3]],["result",6]],[[["formatter",3]],["result",6]],[[["formatter",3]],["result",6]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[]],[[["context",3],["pin",3]],[["poll",4],["result",6]]],[[["pin",3],["context",3]],[["poll",4],["result",6]]],[[["context",3],["pin",3]],[["poll",4],["result",6]]],[[["context",3],["pin",3]],[["result",6],["poll",4]]],[[["context",3],["pin",3]],[["result",6],["poll",4]]],[[],[["result",6],["usize",15]]],[[]],[[]],[[]],[[]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["result",4]],[[],["typeid",3]],[[],["typeid",3]],[[],["typeid",3]],[[],["typeid",3]],[[],[["result",6],["usize",15]]]],"p":[[8,"View"],[3,"Grant"],[3,"Release"],[8,"ViewMut"],[3,"Sink"],[3,"Source"],[3,"Sink"],[3,"Source"],[3,"GrantOverflow"],[4,"Infallible"],[3,"Reader"],[3,"AsyncReader"],[3,"Writer"],[3,"AsyncWriter"]]}\
}');
if (window.initSearch) {window.initSearch(searchIndex)};