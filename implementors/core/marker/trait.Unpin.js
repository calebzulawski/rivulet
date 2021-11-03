(function() {var implementors = {};
implementors["rivulet"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/error/struct.GrantOverflow.html\" title=\"struct rivulet::error::GrantOverflow\">GrantOverflow</a>","synthetic":true,"types":["rivulet::error::GrantOverflow"]},{"text":"impl&lt;T, V&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/slice/struct.Slice.html\" title=\"struct rivulet::slice::Slice\">Slice</a>&lt;T, V&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;V: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["rivulet::slice::Slice"]},{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/splittable/struct.Cloneable.html\" title=\"struct rivulet::splittable::Cloneable\">Cloneable</a>&lt;T&gt;","synthetic":true,"types":["rivulet::splittable::cloneable_source::Cloneable"]},{"text":"impl&lt;'pin, 'a, T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/struct.Grant.html\" title=\"struct rivulet::Grant\">Grant</a>&lt;'a, T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__Grant&lt;'pin, 'a, T&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::base::Grant"]},{"text":"impl&lt;'pin, V, E, F&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/struct.MapError.html\" title=\"struct rivulet::MapError\">MapError</a>&lt;V, E, F&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__MapError&lt;'pin, V, E, F&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::base::MapError"]},{"text":"impl&lt;'pin, T:&nbsp;<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> + 'static&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/circular_buffer/spmc/struct.Sink.html\" title=\"struct rivulet::circular_buffer::spmc::Sink\">Sink</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__Sink&lt;'pin, T&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::circular_buffer::spmc::Sink"]},{"text":"impl&lt;'pin, T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/circular_buffer/spmc/struct.Source.html\" title=\"struct rivulet::circular_buffer::spmc::Source\">Source</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__Source&lt;'pin, T&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::circular_buffer::spmc::Source"]},{"text":"impl&lt;'pin, T:&nbsp;<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> + 'static&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/circular_buffer/spsc/struct.Sink.html\" title=\"struct rivulet::circular_buffer::spsc::Sink\">Sink</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__Sink&lt;'pin, T&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::circular_buffer::spsc::Sink"]},{"text":"impl&lt;'pin, T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/circular_buffer/spsc/struct.Source.html\" title=\"struct rivulet::circular_buffer::spsc::Source\">Source</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__Source&lt;'pin, T&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::circular_buffer::spsc::Source"]},{"text":"impl&lt;'pin, S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/io/struct.Reader.html\" title=\"struct rivulet::io::Reader\">Reader</a>&lt;S&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;S: <a class=\"trait\" href=\"rivulet/trait.Source.html\" title=\"trait rivulet::Source\">Source</a>&lt;Item = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;__Reader&lt;'pin, S&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::io::Reader"]},{"text":"impl&lt;'pin, S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/io/struct.AsyncReader.html\" title=\"struct rivulet::io::AsyncReader\">AsyncReader</a>&lt;S&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;S: <a class=\"trait\" href=\"rivulet/trait.Source.html\" title=\"trait rivulet::Source\">Source</a>&lt;Item = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;__AsyncReader&lt;'pin, S&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::io::AsyncReader"]},{"text":"impl&lt;'pin, S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/io/struct.Writer.html\" title=\"struct rivulet::io::Writer\">Writer</a>&lt;S&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;S: <a class=\"trait\" href=\"rivulet/trait.Sink.html\" title=\"trait rivulet::Sink\">Sink</a>&lt;Item = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;__Writer&lt;'pin, S&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::io::Writer"]},{"text":"impl&lt;'pin, S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/io/struct.AsyncWriter.html\" title=\"struct rivulet::io::AsyncWriter\">AsyncWriter</a>&lt;S&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;S: <a class=\"trait\" href=\"rivulet/trait.Sink.html\" title=\"trait rivulet::Sink\">Sink</a>&lt;Item = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>&gt; + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;__AsyncWriter&lt;'pin, S&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::io::AsyncWriter"]},{"text":"impl&lt;'pin, V, F&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/lazy/struct.Lazy.html\" title=\"struct rivulet::lazy::Lazy\">Lazy</a>&lt;V, F&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__Lazy&lt;'pin, V, F&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::lazy::Lazy"]},{"text":"impl&lt;'pin, Sink, Source, F&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/lazy/struct.LazyChannelSink.html\" title=\"struct rivulet::lazy::LazyChannelSink\">LazyChannelSink</a>&lt;Sink, Source, F&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__LazyChannelSink&lt;'pin, Sink, Source, F&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::lazy::channel::LazyChannelSink"]},{"text":"impl&lt;'pin, Sink, Source, F&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/lazy/struct.LazyChannelSource.html\" title=\"struct rivulet::lazy::LazyChannelSource\">LazyChannelSource</a>&lt;Sink, Source, F&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;__LazyChannelSource&lt;'pin, Sink, Source, F&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::lazy::channel::LazyChannelSource"]},{"text":"impl&lt;'pin, T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"rivulet/splittable/struct.Source.html\" title=\"struct rivulet::splittable::Source\">Source</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"rivulet/splittable/trait.SplittableSource.html\" title=\"trait rivulet::splittable::SplittableSource\">SplittableSource</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;__Source&lt;'pin, T&gt;: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":false,"types":["rivulet::splittable::source::Source"]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()