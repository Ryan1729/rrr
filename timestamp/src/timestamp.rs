use time::OffsetDateTime as ODT;
pub use time::error::IndeterminateOffset;

pub const DEFAULT: Timestamp = Timestamp(ODT::UNIX_EPOCH);

/// A local-time timestamp.
#[derive(Debug)]
#[repr(transparent)]
pub struct Timestamp(ODT);

impl core::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        struct W<'refr, 'f>(&'refr mut core::fmt::Formatter<'f>);

        impl std::io::Write for W<'_, '_> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                let len = buf.len();

                self.0.write_str(
                    std::str::from_utf8(buf)
                        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?
                ).map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;

                Ok(len)
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        self.0.format_into(&mut W(f), &time::format_description::well_known::Rfc3339)
            .map(|_| ())
            .map_err(|_| core::fmt::Error)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        DEFAULT
    }
}

impl Timestamp {
    pub fn now() -> Result<Self, IndeterminateOffset> {
        ODT::now_local().map(Self)
    }
}