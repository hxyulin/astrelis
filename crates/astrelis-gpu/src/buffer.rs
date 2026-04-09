//! Buffer descriptors and usage flags.

/// Usage flags for a GPU buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferUsages(u32);

impl BufferUsages {
    /// Buffer can be mapped for reading.
    pub const MAP_READ: Self = Self(1);
    /// Buffer can be mapped for writing.
    pub const MAP_WRITE: Self = Self(2);
    /// Buffer can be used as a copy source.
    pub const COPY_SRC: Self = Self(4);
    /// Buffer can be used as a copy destination.
    pub const COPY_DST: Self = Self(8);
    /// Buffer can be used as an index buffer.
    pub const INDEX: Self = Self(16);
    /// Buffer can be used as a vertex buffer.
    pub const VERTEX: Self = Self(32);
    /// Buffer can be used as a uniform buffer.
    pub const UNIFORM: Self = Self(64);
    /// Buffer can be used as a storage buffer.
    pub const STORAGE: Self = Self(128);
    /// Buffer can be used for indirect draw/dispatch arguments.
    pub const INDIRECT: Self = Self(256);

    /// Returns `true` if all bits in `other` are set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns the raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for BufferUsages {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Describes a buffer to be created.
#[derive(Clone, Debug)]
pub struct BufferDescriptor<'a> {
    /// Debug label for the buffer.
    pub label: Option<&'a str>,
    /// Size in bytes.
    pub size: u64,
    /// Usage flags.
    pub usage: BufferUsages,
    /// If `true`, the buffer is created mapped for immediate CPU write.
    pub mapped_at_creation: bool,
}

/// Describes a buffer to be created with initial data.
///
/// The buffer size is inferred from `contents.len()`.
#[derive(Clone, Debug)]
pub struct BufferInitDescriptor<'a> {
    /// Debug label for the buffer.
    pub label: Option<&'a str>,
    /// Initial data. Buffer size equals `contents.len()`.
    pub contents: &'a [u8],
    /// Usage flags.
    pub usage: BufferUsages,
}
