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

#[derive(Debug, Clone)]
pub enum TirInstr {
    Const(TirValueId, TirConst),
    Make(TirValueId, TirType, Option<String>, Vec<TirValueId>),
    Get(TirValueId, TirValueId, TirKey),
    Set(TirValueId, TirKey, TirValueId),
    Tag(TirValueId, TirValueId),
    Unwrap(TirValueId, TirValueId, String, u16),

    Slot(TirSlotId, TirType),
    Load(TirValueId, TirSlotId),
    Store(TirSlotId, TirValueId),

    Call(TirValueId, TirFunctionId, Vec<TirValueId>),
    
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
    pub return_type: TirType,
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
