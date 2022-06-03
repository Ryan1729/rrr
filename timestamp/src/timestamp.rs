use time::{OffsetDateTime as ODT, UtcOffset as UO};

/// A local-time timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Timestamp(ODT);

impl Timestamp {
    pub const DEFAULT: Timestamp = Timestamp(ODT::UNIX_EPOCH);
    pub const MAX: Timestamp = Timestamp(
        ODT::UNIX_EPOCH.saturating_add(time::Duration::MAX)
    );
}

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
            .map_err(|_| core::fmt::Error)?;

        // Yes, the Z indicates UTC, but I want it to be more obvious
        if self.0.offset() == time::UtcOffset::UTC {
            write!(f, " UTC")?;
        }

        Ok(())
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Timestamp {
    pub fn now_at_offset(UtcOffset(offset): UtcOffset) -> Self {
        Self(ODT::now_utc().to_offset(offset))
    }
}

/// An offset from UTC. Essentially a timezone, without associted metadata.
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct UtcOffset(UO);

impl UtcOffset {
    // This will currently always return the UTC offset, (that is, offset the time by
    // no offet at all) if there is more than one thread.
    pub fn current_local_or_utc() -> Self {
        Self(
            UO::current_local_offset()
                .unwrap_or(UO::UTC)
        )
    }
}