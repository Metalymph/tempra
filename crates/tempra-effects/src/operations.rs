//! Canonical specialized-effect operation vocabulary.
//!
//! This module is declaration metadata only. It does not parse Tempra syntax,
//! classify TIR, enforce contracts, or claim runtime availability. The closed
//! taxonomy mirrors sections 3 and 4 of `docs/specs/specialized-effects-v0.2.md`.

use crate::Capability;

/// A specialized-effect family under an existing broad capability.
///
/// Families are taxonomy groupings, not capabilities. They do not allocate
/// [`CapabilitySet`](crate::CapabilitySet) bits and do not change an operation's
/// parent capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum EffectFamily {
    /// The `fs` filesystem family.
    Fs = 0,
    /// The `net` network family.
    Net = 1,
}

impl EffectFamily {
    /// Every family in canonical taxonomy order.
    pub const ALL: [Self; 2] = [Self::Fs, Self::Net];

    /// Recognizes an exact family spelling.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "fs" => Some(Self::Fs),
            "net" => Some(Self::Net),
            _ => None,
        }
    }

    /// Returns the stable canonical lower-case name.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Fs => "fs",
            Self::Net => "net",
        }
    }
}

/// A named cross-family category over specialized operations.
///
/// Categories are denial-side predicates. They do not allocate capability bits,
/// change an operation's identity, or add a family level to direct operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum EffectCategory {
    /// The `io.read` category: filesystem reads, network receives and generic reads.
    IoRead = 0,
    /// The `io.write` category: filesystem writes, network sends and generic writes.
    IoWrite = 1,
}

impl EffectCategory {
    /// Every category in canonical taxonomy order.
    pub const ALL: [Self; 2] = [Self::IoRead, Self::IoWrite];

    /// Recognizes an exact category spelling.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "io.read" => Some(Self::IoRead),
            "io.write" => Some(Self::IoWrite),
            _ => None,
        }
    }

    /// Returns the stable canonical dotted name.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::IoRead => "io.read",
            Self::IoWrite => "io.write",
        }
    }
}

/// A specialized effect operation from the closed v0.2 taxonomy.
///
/// The declaration order is stable and is also [`EffectOperation::ALL`]. Every
/// operation refines exactly one existing broad capability; specialized
/// operations are not independent permissions and do not allocate
/// [`CapabilitySet`](crate::CapabilitySet) bits.
///
/// Existing discriminants are preserved. The new `net.send`/`net.receive`
/// variants use previously unused values so K4.9A does not renumber any v0.1
/// operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EffectOperation {
    /// `fs.read`: read filesystem content.
    FsRead = 0,
    /// `fs.write`: create or modify filesystem content.
    FsWrite = 1,
    /// `fs.remove`: delete filesystem content.
    FsRemove = 2,
    /// `net.connect`: establish an outbound connection.
    NetConnect = 3,
    /// `net.listen`: create or bind a listener for inbound connections.
    NetListen = 4,
    /// `net.send`: transmit data over an established connection.
    NetSend = 11,
    /// `net.receive`: receive data from an established connection.
    NetReceive = 12,
    /// `io.read`: read from a console or stream.
    IoRead = 5,
    /// `io.write`: write to a console or stream.
    IoWrite = 6,
    /// `process.exec`: execute an external process.
    ProcessExec = 7,
    /// `process.exit`: terminate the current process.
    ProcessExit = 8,
    /// `spawn.task`: spawn structured concurrent work.
    SpawnTask = 9,
    /// `spawn.actor`: spawn an actor.
    SpawnActor = 10,
}

impl EffectOperation {
    /// Every operation in canonical taxonomy order.
    pub const ALL: [Self; 13] = [
        Self::FsRead,
        Self::FsWrite,
        Self::FsRemove,
        Self::NetConnect,
        Self::NetListen,
        Self::NetSend,
        Self::NetReceive,
        Self::IoRead,
        Self::IoWrite,
        Self::ProcessExec,
        Self::ProcessExit,
        Self::SpawnTask,
        Self::SpawnActor,
    ];

    /// Returns the single broad capability this operation refines.
    pub const fn parent(self) -> Capability {
        match self {
            Self::FsRead | Self::FsWrite | Self::FsRemove => Capability::Io,
            Self::NetConnect | Self::NetListen | Self::NetSend | Self::NetReceive => Capability::Io,
            Self::IoRead | Self::IoWrite => Capability::Io,
            Self::ProcessExec | Self::ProcessExit => Capability::Process,
            Self::SpawnTask | Self::SpawnActor => Capability::Spawn,
        }
    }

    /// Returns the optional family this operation belongs to.
    pub const fn family(self) -> Option<EffectFamily> {
        match self {
            Self::FsRead | Self::FsWrite | Self::FsRemove => Some(EffectFamily::Fs),
            Self::NetConnect | Self::NetListen | Self::NetSend | Self::NetReceive => {
                Some(EffectFamily::Net)
            }
            Self::IoRead
            | Self::IoWrite
            | Self::ProcessExec
            | Self::ProcessExit
            | Self::SpawnTask
            | Self::SpawnActor => None,
        }
    }

    /// Returns the cross-family categories this operation belongs to.
    pub const fn categories(self) -> &'static [EffectCategory] {
        match self {
            Self::FsRead | Self::NetReceive | Self::IoRead => &[EffectCategory::IoRead],
            Self::FsWrite | Self::NetSend | Self::IoWrite => &[EffectCategory::IoWrite],
            Self::FsRemove
            | Self::NetConnect
            | Self::NetListen
            | Self::ProcessExec
            | Self::ProcessExit
            | Self::SpawnTask
            | Self::SpawnActor => &[],
        }
    }

    /// Returns whether this operation belongs to `category`.
    pub fn is_in_category(self, category: EffectCategory) -> bool {
        self.categories().contains(&category)
    }

    /// Returns the stable canonical dotted name.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::FsRead => "fs.read",
            Self::FsWrite => "fs.write",
            Self::FsRemove => "fs.remove",
            Self::NetConnect => "net.connect",
            Self::NetListen => "net.listen",
            Self::NetSend => "net.send",
            Self::NetReceive => "net.receive",
            Self::IoRead => "io.read",
            Self::IoWrite => "io.write",
            Self::ProcessExec => "process.exec",
            Self::ProcessExit => "process.exit",
            Self::SpawnTask => "spawn.task",
            Self::SpawnActor => "spawn.actor",
        }
    }

    /// Returns the parameter keys accepted syntactically for this operation.
    ///
    /// This is declaration metadata only. It does not assert that the operation,
    /// any parameter, or any clause is implemented, classified, enforceable or
    /// reachable. In particular, [`EffectParameter::Scope`] remains deferred for
    /// `spawn.task`/`spawn.actor` and does not imply runtime support.
    pub const fn valid_parameter_keys(self) -> &'static [EffectParameter] {
        match self {
            Self::FsRead => &[EffectParameter::Source],
            Self::FsWrite | Self::FsRemove => &[EffectParameter::Destination],
            Self::NetConnect | Self::NetListen => &[
                EffectParameter::Host,
                EffectParameter::Port,
                EffectParameter::Protocol,
            ],
            Self::NetSend | Self::NetReceive => &[],
            Self::IoRead | Self::IoWrite => &[EffectParameter::Stream],
            Self::ProcessExec => &[EffectParameter::Executable],
            Self::ProcessExit => &[],
            Self::SpawnTask | Self::SpawnActor => &[EffectParameter::Scope],
        }
    }

    /// Returns whether `parameter` is accepted syntactically by this operation.
    ///
    /// This is a metadata predicate, not an enforcement or availability check.
    pub fn accepts_parameter(self, parameter: EffectParameter) -> bool {
        self.valid_parameter_keys().contains(&parameter)
    }
}

/// A typed parameter key accepted by specialized effect clauses.
///
/// The vocabulary is closed and its declaration order is stable. Parameters are
/// compared statically from declaration-site literals; this type carries no
/// values and makes no runtime guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EffectParameter {
    /// `source`: an opaque path spelling read by the operation.
    Source = 0,
    /// `destination`: an opaque path spelling created, modified or removed.
    Destination = 1,
    /// `host`: an opaque host name compared as written.
    Host = 2,
    /// `port`: a numeric port.
    Port = 3,
    /// `protocol`: a transport protocol literal.
    Protocol = 4,
    /// `executable`: an opaque executable spelling.
    Executable = 5,
    /// `stream`: a console or standard stream literal.
    Stream = 6,
    /// `scope`: a statically named grouping label.
    ///
    /// Deferred with `spawn.*`; see [`EffectOperation::valid_parameter_keys`].
    Scope = 7,
}

impl EffectParameter {
    /// Every parameter key in canonical taxonomy order.
    pub const ALL: [Self; 8] = [
        Self::Source,
        Self::Destination,
        Self::Host,
        Self::Port,
        Self::Protocol,
        Self::Executable,
        Self::Stream,
        Self::Scope,
    ];

    /// Returns the stable canonical lower-case name.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Destination => "destination",
            Self::Host => "host",
            Self::Port => "port",
            Self::Protocol => "protocol",
            Self::Executable => "executable",
            Self::Stream => "stream",
            Self::Scope => "scope",
        }
    }
}
