/// A simple and not the most-efficient fixed size string on the stack.
///
/// This supplanted some uses of `Box<str>` for storing tiny strings in an
/// effort to reduce our dependence on dynamic memory allocation.
///
/// Also, since it isn't needed and it lets us save on storage requirements,
/// `N` must be less than `256` (so that the length can fit in a `u8`).
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct ArrayStr<const N: usize> {
    /// The UTF-8 bytes that make up the string.
    ///
    /// This array---the entire array---is always valid UTF-8. And
    /// the `0..self.len` sub-slice is also always valid UTF-8.
    bytes: [u8; N],
    /// The number of bytes used by the string in `bytes`.
    ///
    /// (We could technically save this byte in some cases and use a NUL
    /// terminator. For example, since we don't permit NUL bytes in POSIX time
    /// zone abbreviation strings, but this is simpler and only one byte and
    /// generalizes. And we're not really trying to micro-optimize the storage
    /// requirements when we use these array strings. Or at least, I don't know
    /// of a reason to.)
    len: u8,
}

impl<const N: usize> ArrayStr<N> {
    /// Creates a new fixed capacity string.
    ///
    /// If the given string exceeds `N` bytes, then this returns
    /// `None`.
    pub(crate) fn new(s: &str) -> Option<ArrayStr<N>> {
        let len = s.len();
        if len > N {
            return None;
        }
        let mut bytes = [0; N];
        bytes[..len].copy_from_slice(s.as_bytes());
        // OK because ABBREVIATION_MAX will never exceed u8::MAX.
        debug_assert!(
            N <= usize::from(u8::MAX),
            "size of ArrayStr is too big"
        );
        let len = u8::try_from(len).unwrap();
        Some(ArrayStr { bytes, len })
    }

    /// Returns this array string as a string slice.
    pub(crate) fn as_str(&self) -> &str {
        // OK because construction guarantees valid UTF-8.
        //
        // This is bullet proof enough to use unchecked `str` construction
        // here, but I can't dream up of a benchmark where it matters.
        core::str::from_utf8(&self.bytes[..usize::from(self.len)]).unwrap()
    }
}

/// Easy construction of `ArrayStr` from `&'static str`.
///
/// We specifically limit to `&'static str` to approximate string literals.
/// This prevents most cases of accidentally creating a non-string literal
/// that panics if the string is too big.
///
/// This impl primarily exists to make writing tests more convenient.
impl<const N: usize> From<&'static str> for ArrayStr<N> {
    fn from(s: &'static str) -> ArrayStr<N> {
        ArrayStr::new(s).unwrap()
    }
}

impl<const N: usize> PartialEq<str> for ArrayStr<N> {
    fn eq(&self, rhs: &str) -> bool {
        self.as_str() == rhs
    }
}

impl<const N: usize> PartialEq<&str> for ArrayStr<N> {
    fn eq(&self, rhs: &&str) -> bool {
        self.as_str() == *rhs
    }
}

impl<const N: usize> PartialEq<ArrayStr<N>> for str {
    fn eq(&self, rhs: &ArrayStr<N>) -> bool {
        self == rhs.as_str()
    }
}

impl<const N: usize> core::fmt::Debug for ArrayStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl<const N: usize> core::fmt::Display for ArrayStr<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self.as_str(), f)
    }
}
