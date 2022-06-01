use atomicwrites::{AllowOverwrite, AtomicFile};

pub use atomicwrites::Error;

pub fn write_atomically<P, F, E>(
    path: P,
    f: F
) -> Result<(), Error<E>>
where
    P: AsRef<std::path::Path>,
    F: FnOnce(&mut std::fs::File) -> Result<(), E>
{
    AtomicFile::new(path, AllowOverwrite).write(f)
}