/// A capability tracked by Tempra's internal semantic analysis.
///
/// The declaration order is stable and is also the iteration order used by
/// [`CapabilitySet`]. `Unsafe` is internal and is not a public `@deny` name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Capability {
    /// Runtime allocation may be required.
    Alloc = 0,
    /// A runtime operation may panic or otherwise fail partially.
    Panic = 1,
    /// Input or output may be performed.
    Io = 2,
    /// Process creation or process control may be performed.
    Process = 3,
    /// Concurrent work may be spawned.
    Spawn = 4,
    /// A nondeterministic value or observation may be produced.
    Nondeterminism = 5,
    /// A low-level operation outside normal safe Tempra may be performed.
    Unsafe = 6,
}

impl Capability {
    const ALL: [Self; 7] = [
        Self::Alloc,
        Self::Panic,
        Self::Io,
        Self::Process,
        Self::Spawn,
        Self::Nondeterminism,
        Self::Unsafe,
    ];

    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// A compact set of internal Tempra capabilities.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CapabilitySet(u8);

impl CapabilitySet {
    /// Creates an empty capability set.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates a set containing exactly one capability.
    pub const fn singleton(capability: Capability) -> Self {
        Self(capability.bit())
    }

    /// Returns whether the set contains `capability`.
    pub const fn contains(self, capability: Capability) -> bool {
        self.0 & capability.bit() != 0
    }

    /// Inserts a capability and returns whether the set changed.
    pub fn insert(&mut self, capability: Capability) -> bool {
        let previous = self.0;
        self.0 |= capability.bit();
        self.0 != previous
    }

    /// Returns the union of two capability sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Iterates over contained capabilities in stable declaration order.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .into_iter()
            .filter(move |capability| self.contains(*capability))
    }
}
