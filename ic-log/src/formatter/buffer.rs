use std::io;

// A buffer to store log formatted data
#[derive(Default)]
pub struct Buffer(Vec<u8>);

impl Buffer {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.extend(buf);
        Ok(buf.len())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl From<String> for Buffer {
    fn from(value: String) -> Self {
        Buffer(value.into_bytes())
    }
}

impl From<&str> for Buffer {
    fn from(value: &str) -> Self {
        Buffer(value.as_bytes().to_owned())
    }
}
