//! Canonical specialized-effect operation vocabulary.
//!
//! This module is declaration metadata only. It does not parse Tempra syntax,
//! classify TIR, enforce contracts, or claim runtime availability. The closed
//! taxonomy mirrors sections 3 and 4 of `docs/specs/specialized-effects-v0.1.md`.

use crate::Capability;

/// A specialized effect operation from the closed v0.1 taxonomy.
///
/// The declaration order is stable and is also [`EffectOperation::ALL`]. Every
/// operation refines exactly one existing broad capability; specialized
/// operations are not independent permissions and do not allocate
/// [`CapabilitySet`](crate::CapabilitySet) bits.
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
    /// `net.listen`: accept inbound connections.
    NetListen = 4,
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
    pub const ALL: [Self; 11] = [
        Self::FsRead,
        Self::FsWrite,
        Self::FsRemove,
        Self::NetConnect,
        Self::NetListen,
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
            Self::NetConnect | Self::NetListen => Capability::Io,
            Self::IoRead | Self::IoWrite => Capability::Io,
            Self::ProcessExec | Self::ProcessExit => Capability::Process,
            Self::SpawnTask | Self::SpawnActor => Capability::Spawn,
        }
    }

    /// Returns the stable canonical dotted name.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::FsRead => "fs.read",
            Self::FsWrite => "fs.write",
            Self::FsRemove => "fs.remove",
            Self::NetConnect => "net.connect",
            Self::NetListen => "net.listen",
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
