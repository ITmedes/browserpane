use std::io;
use std::process::ExitStatus;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;

pub(crate) struct BoundedProcessOutput {
    pub(crate) status: ExitStatus,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) stdout_omitted_bytes: u64,
    pub(crate) stderr_omitted_bytes: u64,
}

impl BoundedProcessOutput {
    pub(crate) fn omitted_bytes(&self) -> u64 {
        self.stdout_omitted_bytes
            .saturating_add(self.stderr_omitted_bytes)
    }
}

pub(crate) async fn wait_with_bounded_output(
    mut child: Child,
    max_bytes_per_stream: usize,
) -> io::Result<BoundedProcessOutput> {
    if max_bytes_per_stream == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "worker output limit must be greater than zero",
        ));
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (status, stdout, stderr) = tokio::join!(
        child.wait(),
        read_bounded(stdout, max_bytes_per_stream),
        read_bounded(stderr, max_bytes_per_stream),
    );
    let stdout = stdout?;
    let stderr = stderr?;
    Ok(BoundedProcessOutput {
        status: status?,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_omitted_bytes: stdout.omitted_bytes,
        stderr_omitted_bytes: stderr.omitted_bytes,
    })
}

struct CapturedBytes {
    bytes: Vec<u8>,
    omitted_bytes: u64,
}

async fn read_bounded<R>(reader: Option<R>, max_bytes: usize) -> io::Result<CapturedBytes>
where
    R: AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return Ok(CapturedBytes {
            bytes: Vec::new(),
            omitted_bytes: 0,
        });
    };
    let mut capture = BoundedBytes::new(max_bytes);
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        capture.append(&chunk[..read]);
    }
    Ok(capture.finish())
}

struct BoundedBytes {
    bytes: Vec<u8>,
    length: usize,
    write_offset: usize,
    total_bytes: u64,
}

impl BoundedBytes {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: vec![0; max_bytes],
            length: 0,
            write_offset: 0,
            total_bytes: 0,
        }
    }

    fn append(&mut self, source: &[u8]) {
        self.total_bytes = self
            .total_bytes
            .saturating_add(u64::try_from(source.len()).unwrap_or(u64::MAX));
        if source.len() >= self.bytes.len() {
            let start = source.len() - self.bytes.len();
            self.bytes.copy_from_slice(&source[start..]);
            self.length = self.bytes.len();
            self.write_offset = 0;
            return;
        }

        let first_length = source.len().min(self.bytes.len() - self.write_offset);
        self.bytes[self.write_offset..self.write_offset + first_length]
            .copy_from_slice(&source[..first_length]);
        if first_length < source.len() {
            self.bytes[..source.len() - first_length].copy_from_slice(&source[first_length..]);
        }
        self.write_offset = (self.write_offset + source.len()) % self.bytes.len();
        self.length = self.bytes.len().min(self.length + source.len());
    }

    fn finish(self) -> CapturedBytes {
        let bytes = if self.length < self.bytes.len() {
            self.bytes[..self.length].to_vec()
        } else {
            let mut ordered = Vec::with_capacity(self.length);
            ordered.extend_from_slice(&self.bytes[self.write_offset..]);
            ordered.extend_from_slice(&self.bytes[..self.write_offset]);
            ordered
        };
        CapturedBytes {
            bytes,
            omitted_bytes: self
                .total_bytes
                .saturating_sub(u64::try_from(self.length).unwrap_or(u64::MAX)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedBytes;

    #[test]
    fn retains_complete_output_at_limit() {
        let mut capture = BoundedBytes::new(5);
        capture.append(b"hello");

        let result = capture.finish();
        assert_eq!(result.bytes, b"hello");
        assert_eq!(result.omitted_bytes, 0);
    }

    #[test]
    fn retains_tail_across_ring_wraps() {
        let mut capture = BoundedBytes::new(5);
        capture.append(b"abc");
        capture.append(b"def");
        capture.append(b"gh");

        let result = capture.finish();
        assert_eq!(result.bytes, b"defgh");
        assert_eq!(result.omitted_bytes, 3);
    }

    #[test]
    fn retains_tail_from_chunk_larger_than_limit() {
        let mut capture = BoundedBytes::new(4);
        capture.append(b"abcdefgh");

        let result = capture.finish();
        assert_eq!(result.bytes, b"efgh");
        assert_eq!(result.omitted_bytes, 4);
    }
}
