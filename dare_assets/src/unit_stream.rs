
use futures_util::Stream;
use futures_util::*;

/// Responsible for taking in a stream of unstructured bytes, and shaping them into [Format; chunk_size] chunks streamed out
#[derive(Debug, Clone, Hash)]
pub struct ByteStreamReshaper<InStream: Stream<Item = In> + Unpin, In: AsRef<[u8]>> {
    incoming_stream: InStream,
    incoming_format: crate::Format,
    max_elements: u64,
    /// If [`None`], always try to send regardless of how many elements can make up a chunk
    chunk_elements: Option<u64>,
    /// If [`None`], outgoing bytes are assumed to be the same as incoming, and any transmutation step is unnecessary
    outgoing_format: Option<crate::Format>,

    // stateful
    buffered_bytes: Vec<u8>,
    end_of_stream: bool,
}

impl<InStream: Stream<Item = In> + Unpin, In: AsRef<[u8]>> ByteStreamReshaper<InStream, In> {
    fn new(
        stream: InStream,
        incoming: crate::Format,
        max_elements: u64,
        elements_in_chunk: Option<u64>,
        outgoing: Option<crate::Format>,
    ) -> Self {
        Self {
            incoming_stream: stream,
            max_elements,
            chunk_elements: elements_in_chunk,
            incoming_format: incoming,
            outgoing_format: outgoing,

            buffered_bytes: Vec::new(),
            end_of_stream: false,
        }
    }

    fn take_input_elems(&mut self, elems: usize) -> Vec<u8> {
        let n = elems
            .checked_mul(self.incoming_format.size_in_bytes())
            .expect("Chunk byte overflow");
        self.buffered_bytes.drain(..n).collect()
    }

    fn decide_emit(&mut self) -> Option<Vec<u8>> {
        if self.max_elements == 0 {
            None
        } else {
            let buffered_elems: usize = self
                .buffered_bytes
                .len()
                .div_euclid(self.incoming_format.size_in_bytes());
            let to_remove = (buffered_elems as u64).min(self.max_elements);
            self.max_elements = self.max_elements.saturating_sub(to_remove);
            if to_remove == 0 {
                None
            } else if let Some(buffered) = self.chunk_elements
                && buffered_elems > buffered as usize
            {
                Some(self.take_input_elems(to_remove as usize))
            } else if self.chunk_elements.is_none() {
                Some(self.take_input_elems(to_remove as usize))
            } else {
                None
            }
        }
    }
}

impl<InStream: Stream<Item = In> + Unpin, In: AsRef<[u8]>> Stream
    for ByteStreamReshaper<InStream, In>
{
    type Item = Vec<u8>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            if self.end_of_stream
                && let Some(out) = self.decide_emit()
            {
                return std::task::Poll::Ready(Some(out));
            } else if self.end_of_stream {
                return std::task::Poll::Ready(None);
            } else {
                match self.incoming_stream.poll_next_unpin(cx) {
                    std::task::Poll::Ready(Some(piece)) => {
                        let b: &[u8] = piece.as_ref();
                        if !b.is_empty() {
                            self.buffered_bytes.extend_from_slice(b);
                        }
                        if let Some(elems) = self.decide_emit() {
                            return std::task::Poll::Ready(Some(elems));
                        }
                    }
                    std::task::Poll::Ready(None) => {
                        // end of input stream
                        self.end_of_stream = true;
                        if self.chunk_elements.is_none() {
                            return std::task::Poll::Ready(self.decide_emit());
                        }
                    }
                    std::task::Poll::Pending => {
                        return std::task::Poll::Pending;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {}
