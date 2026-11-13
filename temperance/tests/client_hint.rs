//! Tests for the response-length hint passed to client `Io` implementations

use postcard_schema_ng::key::Key;

use temperance::interface;
use temperance::{
    ClientIoError,
    io::{
        Storage, StorageView,
        client::{Backend, Io},
    },
    wire::HEADER_LEN,
};

interface! {
    mod hinted {
        fn small(u8) -> u8;
        fn word(u8) -> u32;
        fn unit(u8) -> ();
        fn unbounded(u8) -> String;
    }
}

use hinted::Client as _;

const BUF_LEN: usize = 64;

struct Bufs {
    rqst: [u8; BUF_LEN],
    resp: [u8; BUF_LEN],
}

impl Storage for Bufs {
    fn buffers(&mut self) -> StorageView<'_> {
        StorageView {
            rqst_buf: &mut self.rqst,
            resp_buf: &mut self.resp,
        }
    }
}

/// Records the hint and buffer length of each exchange, then fails it
#[derive(Default)]
struct HintIo {
    hint: Option<Option<usize>>,
    incoming_len: Option<usize>,
}

impl Io for HintIo {
    type Error = ();

    fn send_then_receive_raw_frames<'a>(
        &mut self,
        _outgoing: &[u8],
        incoming: &'a mut [u8],
    ) -> Result<&'a [u8], ()> {
        self.incoming_len = Some(incoming.len());
        Err(())
    }

    fn send_then_receive_raw_frames_hinted<'a>(
        &mut self,
        _outgoing: &[u8],
        incoming: &'a mut [u8],
        max_resp_frame: Option<usize>,
    ) -> Result<&'a [u8], ()> {
        self.hint = Some(max_resp_frame);
        self.incoming_len = Some(incoming.len());
        Err(())
    }
}

/// Doesn't override the hinted method
#[derive(Default)]
struct PlainIo {
    incoming_len: Option<usize>,
}

impl Io for PlainIo {
    type Error = ();

    fn send_then_receive_raw_frames<'a>(
        &mut self,
        _outgoing: &[u8],
        incoming: &'a mut [u8],
    ) -> Result<&'a [u8], ()> {
        self.incoming_len = Some(incoming.len());
        Err(())
    }
}

struct TestBackend<I> {
    bufs: Bufs,
    io: I,
}

impl<I: Io> TestBackend<I> {
    fn new(io: I) -> Self {
        Self {
            bufs: Bufs {
                rqst: [0; BUF_LEN],
                resp: [0; BUF_LEN],
            },
            io,
        }
    }
}

impl<I: Io> Backend for TestBackend<I> {
    type Storage = Bufs;
    type Io = I;

    fn next_sequence_number(&mut self) -> u16 {
        0
    }

    fn parts(&mut self) -> (&mut Bufs, &mut I) {
        (&mut self.bufs, &mut self.io)
    }
}

#[test]
fn typed_calls_pass_the_method_max() {
    let mut b = TestBackend::new(HintIo::default());

    assert!(matches!(b.small(&0), Err(ClientIoError::Io(()))));
    assert_eq!(b.io.hint, Some(Some(HEADER_LEN + 1)));

    assert!(matches!(b.word(&0), Err(ClientIoError::Io(()))));
    assert_eq!(b.io.hint, Some(Some(HEADER_LEN + 5)));

    // Room for a `WireError` body, even though `()` is zero bytes
    assert!(matches!(b.unit(&0), Err(ClientIoError::Io(()))));
    assert_eq!(b.io.hint, Some(Some(HEADER_LEN + 1)));

    assert!(matches!(b.unbounded(&0), Err(ClientIoError::Io(()))));
    assert_eq!(b.io.hint, Some(None));

    // The hint never shrinks the buffer itself
    assert_eq!(b.io.incoming_len, Some(BUF_LEN));
}

#[test]
fn raw_calls_have_no_hint() {
    let mut b = TestBackend::new(HintIo::default());
    let res = b.send_then_receive_raw(Key::from_bytes([1; 8]), &[]);
    assert!(matches!(res, Err(ClientIoError::Io(()))));
    assert_eq!(b.io.hint, Some(None));
}

#[test]
fn default_ignores_the_hint() {
    let mut b = TestBackend::new(PlainIo::default());
    assert!(matches!(b.small(&0), Err(ClientIoError::Io(()))));
    assert_eq!(b.io.incoming_len, Some(BUF_LEN));
}
