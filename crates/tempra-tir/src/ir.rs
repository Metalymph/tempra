use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TirValueId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TirBlockId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TirFunctionId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TirSlotId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TirType {
    Unit,
    Bool,
    Int64,
    UInt64,
    UInt8,
    Float64,
    String,
    Never,
    Tag,
    Array(Box<TirType>),
    Slice(Box<TirType>),
    Tuple(Vec<TirType>),
    Struct(String),
    Enum(String),
    Option(Box<TirType>),
    Result(Box<TirType>, Box<TirType>),
    Function(Vec<TirType>, Box<TirType>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TirConst {
    Unit,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TirKey {
    Field(String),
    Index(usize),
}

/// Origin of an exclusive array view. Parameter indices refer to an incoming
/// exclusive slice, not to a value materialized from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TirSliceSource {
    Owner(TirSlotId),
    Parameter(usize),
}

#[derive(Debug, Clone)]
pub enum TirInstr {
    Const(TirValueId, TirConst),
    Make(TirValueId, TirType, Option<String>, Vec<TirValueId>),
    MakeStruct(TirValueId, String, Vec<(String, TirValueId)>), // dest, struct_name, fields
    Get(TirValueId, TirValueId, TirKey),
    Set(TirValueId, TirValueId, TirKey, TirValueId), // dest, object, key, value
    Tag(TirValueId, TirValueId),
    Unwrap(TirValueId, TirValueId, String, u16),

    Slot(TirSlotId, TirType),
    /// Bind a declared slot to an incoming non-slice inout parameter's storage.
    /// This is a place binding, not a copy of the parameter's value.
    BindInout(TirSlotId, usize),
    Load(TirValueId, TirSlotId),
    /// Materialize an exclusive storage operand only at a verified direct call.
    /// It must not be used as an ordinary SSA value or escape the call.
    BorrowInout(TirValueId, TirSlotId),
    /// Exclusive subview consumed by a direct call; bounds are relative to source.
    BorrowSliceInout(TirValueId, TirSliceSource, TirValueId, TirValueId),
    /// Access an element or length of an incoming exclusive slice parameter.
    InoutSliceGet(TirValueId, usize, TirValueId),
    InoutSliceSet(usize, TirValueId, TirValueId),
    InoutSliceLen(TirValueId, usize),
    Store(TirSlotId, TirValueId),

    Call(TirValueId, TirFunctionId, Vec<TirValueId>),
    // The referenced function receives captured values first, then call arguments.
    // The captured environment may be empty and requires no heap allocation.
    MakeClosure(TirValueId, TirFunctionId, Vec<TirValueId>),
    // VM Parity 2B.1b: borrowed capture representation (not yet executable).
    // Unlike MakeClosure, the environment names ORIGINAL storage slots; no
    // captured value is copied or moved by this instruction. Borrow lifetimes
    // and non-escaping enforcement belong to the later capture lowering gate.
    MakeClosureRef(TirValueId, TirFunctionId, Vec<TirSlotId>),
    /// Exclusive, non-owning captured storage. Only mutated through its owner handle.
    /// A separate instruction prevents read-only borrows from being silently upgraded.
    MakeClosureInout(TirValueId, TirFunctionId, Vec<TirSlotId>),
    CallClosure(TirValueId, TirValueId, Vec<TirValueId>),

    Add(TirValueId, TirValueId, TirValueId),
    Sub(TirValueId, TirValueId, TirValueId),
    Mul(TirValueId, TirValueId, TirValueId),
    Div(TirValueId, TirValueId, TirValueId),
    Eq(TirValueId, TirValueId, TirValueId),
    Lt(TirValueId, TirValueId, TirValueId),
    Gt(TirValueId, TirValueId, TirValueId),
    And(TirValueId, TirValueId, TirValueId),
    Or(TirValueId, TirValueId, TirValueId),
    Not(TirValueId, TirValueId),

    // Array/Slice dynamic operations
    GetIndex(TirValueId, TirValueId, TirValueId), // dest, object, index
    Slice(TirValueId, TirValueId, TirValueId, TirValueId), // dest, object, start, end
    Len(TirValueId, TirValueId),                  // dest, object
    Size(TirValueId, TirValueId),                 // dest, object
    Bytes(TirValueId, TirValueId),                // dest, object
    ArrayPush(TirValueId, TirValueId, TirValueId), // dest, array, value — produces new array with value appended
}

#[derive(Debug, Clone)]
pub enum TirTerminator {
    Br(TirBlockId),
    CondBr(TirValueId, TirBlockId, TirBlockId),
    Ret(Option<TirValueId>),
    Unreachable,
}

#[derive(Debug, Clone)]
pub struct TirBlock {
    pub id: TirBlockId,
    pub instrs: Vec<TirInstr>,
    pub terminator: Option<TirTerminator>,
}

#[derive(Debug, Clone)]
pub struct TirFunction {
    pub id: TirFunctionId,
    pub name: String,
    pub params: Vec<(TirValueId, TirType)>,
    /// Access modes, one per runtime parameter. Never a user-visible type.
    pub inout_params: Vec<bool>,
    pub return_type: TirType,
    pub is_extern: bool,
    pub blocks: Vec<TirBlock>,
    pub next_value_id: usize,
    pub next_block_id: usize,
    pub next_slot_id: usize,
    pub debug_slots: HashMap<TirSlotId, String>,
}

#[derive(Debug, Clone, Default)]
pub struct TirModule {
    pub functions: Vec<TirFunction>,
}
