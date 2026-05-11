use crate::bytecode::BytecodeModule;
use crate::constant::{BytecodeConstant, ConstId};
use crate::function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
use crate::opcode::{BytecodeInstr, InstrOffset};
use crate::verify::verify_module;
use std::fmt;
use std::io::{self, Read, Write};

#[derive(Debug, PartialEq)]
pub enum BytecodeSerializeError {
    InvalidMagic,
    UnsupportedVersion,
    UnexpectedEof,
    InvalidTag,
    Utf8Error,
    TrailingBytes,
    VerifyError(crate::verify::BytecodeVerifyError),
    IoError(String),
    Message(String),
}

impl fmt::Display for BytecodeSerializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BytecodeSerializeError::InvalidMagic => write!(f, "Invalid magic number"),
            BytecodeSerializeError::UnsupportedVersion => write!(f, "Unsupported format version"),
            BytecodeSerializeError::UnexpectedEof => write!(f, "Unexpected end of file"),
            BytecodeSerializeError::InvalidTag => write!(f, "Invalid enum tag"),
            BytecodeSerializeError::Utf8Error => write!(f, "Invalid UTF-8 sequence"),
            BytecodeSerializeError::TrailingBytes => write!(f, "Trailing bytes after decode"),
            BytecodeSerializeError::VerifyError(e) => write!(f, "Verification error: {}", e),
            BytecodeSerializeError::IoError(e) => write!(f, "IO error: {}", e),
            BytecodeSerializeError::Message(m) => write!(f, "{}", m),
        }
    }
}

impl std::error::Error for BytecodeSerializeError {}

impl From<io::Error> for BytecodeSerializeError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            BytecodeSerializeError::UnexpectedEof
        } else {
            BytecodeSerializeError::IoError(e.to_string())
        }
    }
}

impl From<std::string::FromUtf8Error> for BytecodeSerializeError {
    fn from(_: std::string::FromUtf8Error) -> Self {
        BytecodeSerializeError::Utf8Error
    }
}

impl From<crate::verify::BytecodeVerifyError> for BytecodeSerializeError {
    fn from(e: crate::verify::BytecodeVerifyError) -> Self {
        BytecodeSerializeError::VerifyError(e)
    }
}

const MAGIC: &[u8; 4] = b"TMBC";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeCacheHeader {
    pub format_version: u32,
    pub compiler_fingerprint: String,
    pub source_hash: [u8; 32],
    pub language_version: String,
    pub stdlib_abi_version: String,
}

impl Default for BytecodeCacheHeader {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            compiler_fingerprint: "unknown".to_string(),
            source_hash: [0; 32],
            language_version: "0.1.0".to_string(),
            stdlib_abi_version: "0.1.0".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BytecodeCacheArtifact {
    pub header: BytecodeCacheHeader,
    pub module: BytecodeModule,
}

// --- Encoding ---

pub fn encode_module(module: &BytecodeModule) -> Result<Vec<u8>, BytecodeSerializeError> {
    let artifact = BytecodeCacheArtifact {
        header: BytecodeCacheHeader::default(),
        module: module.clone(),
    };
    encode_artifact(&artifact)
}

pub fn encode_artifact(artifact: &BytecodeCacheArtifact) -> Result<Vec<u8>, BytecodeSerializeError> {
    let mut buf = Vec::new();
    
    // Magic
    buf.write_all(MAGIC)?;
    
    // Header
    write_u32(&mut buf, artifact.header.format_version)?;
    write_string(&mut buf, &artifact.header.compiler_fingerprint)?;
    buf.write_all(&artifact.header.source_hash)?;
    write_string(&mut buf, &artifact.header.language_version)?;
    write_string(&mut buf, &artifact.header.stdlib_abi_version)?;
    
    // Module
    write_u32(&mut buf, artifact.module.version)?;
    
    // Constants
    write_u32(&mut buf, artifact.module.constants.len() as u32)?;
    for c in &artifact.module.constants {
        match c {
            BytecodeConstant::Unit => buf.write_all(&[0])?,
            BytecodeConstant::Bool(b) => {
                buf.write_all(&[1])?;
                buf.write_all(&[*b as u8])?;
            }
            BytecodeConstant::Int64(i) => {
                buf.write_all(&[2])?;
                write_i64(&mut buf, *i)?;
            }
            BytecodeConstant::UInt64(u) => {
                buf.write_all(&[3])?;
                write_u64(&mut buf, *u)?;
            }
            BytecodeConstant::Float64(f) => {
                buf.write_all(&[4])?;
                write_u64(&mut buf, f.to_bits())?;
            }
            BytecodeConstant::String(s) => {
                buf.write_all(&[5])?;
                write_string(&mut buf, s)?;
            }
        }
    }
    
    // Functions
    write_u32(&mut buf, artifact.module.functions.len() as u32)?;
    for f in &artifact.module.functions {
        write_string(&mut buf, &f.name)?;
        
        write_u32(&mut buf, f.params.len() as u32)?;
        for p in &f.params {
            write_type(&mut buf, p)?;
        }
        
        write_type(&mut buf, &f.ret)?;
        
        write_u32(&mut buf, f.locals.len() as u32)?;
        for l in &f.locals {
            write_type(&mut buf, l)?;
        }
        
        write_u32(&mut buf, f.max_stack)?;
        
        write_u32(&mut buf, f.code.len() as u32)?;
        for instr in &f.code {
            write_instr(&mut buf, instr)?;
        }
    }
    
    Ok(buf)
}

fn write_u32(w: &mut impl Write, v: u32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_u64(w: &mut impl Write, v: u64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_i64(w: &mut impl Write, v: i64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_string(w: &mut impl Write, s: &str) -> io::Result<()> {
    let bytes = s.as_bytes();
    write_u32(w, bytes.len() as u32)?;
    w.write_all(bytes)
}

fn write_type(w: &mut impl Write, ty: &BytecodeType) -> io::Result<()> {
    let tag = match ty {
        BytecodeType::Unit => 0,
        BytecodeType::Bool => 1,
        BytecodeType::Int64 => 2,
        BytecodeType::UInt64 => 3,
        BytecodeType::Float64 => 4,
        BytecodeType::String => 5,
        BytecodeType::Any => 6,
    };
    w.write_all(&[tag])
}

fn write_instr(w: &mut impl Write, instr: &BytecodeInstr) -> io::Result<()> {
    match instr {
        BytecodeInstr::ConstUnit => w.write_all(&[0]),
        BytecodeInstr::ConstBool(b) => {
            w.write_all(&[1, *b as u8])
        }
        BytecodeInstr::ConstI64(id) => {
            w.write_all(&[2])?;
            write_u32(w, id.0)
        }
        BytecodeInstr::ConstU64(id) => {
            w.write_all(&[3])?;
            write_u32(w, id.0)
        }
        BytecodeInstr::ConstF64(id) => {
            w.write_all(&[4])?;
            write_u32(w, id.0)
        }
        BytecodeInstr::ConstString(id) => {
            w.write_all(&[5])?;
            write_u32(w, id.0)
        }
        BytecodeInstr::LoadLocal(id) => {
            w.write_all(&[6])?;
            write_u32(w, id.0)
        }
        BytecodeInstr::StoreLocal(id) => {
            w.write_all(&[7])?;
            write_u32(w, id.0)
        }
        BytecodeInstr::Pop => w.write_all(&[8]),
        BytecodeInstr::Dup => w.write_all(&[9]),
        BytecodeInstr::AddI64 => w.write_all(&[10]),
        BytecodeInstr::SubI64 => w.write_all(&[11]),
        BytecodeInstr::MulI64 => w.write_all(&[12]),
        BytecodeInstr::DivI64 => w.write_all(&[13]),
        BytecodeInstr::AddU64 => w.write_all(&[14]),
        BytecodeInstr::SubU64 => w.write_all(&[15]),
        BytecodeInstr::MulU64 => w.write_all(&[16]),
        BytecodeInstr::DivU64 => w.write_all(&[17]),
        BytecodeInstr::AddF64 => w.write_all(&[18]),
        BytecodeInstr::SubF64 => w.write_all(&[19]),
        BytecodeInstr::MulF64 => w.write_all(&[20]),
        BytecodeInstr::DivF64 => w.write_all(&[21]),
        BytecodeInstr::Eq => w.write_all(&[22]),
        BytecodeInstr::LtI64 => w.write_all(&[23]),
        BytecodeInstr::GtI64 => w.write_all(&[24]),
        BytecodeInstr::LtU64 => w.write_all(&[25]),
        BytecodeInstr::GtU64 => w.write_all(&[26]),
        BytecodeInstr::LtF64 => w.write_all(&[27]),
        BytecodeInstr::GtF64 => w.write_all(&[28]),
        BytecodeInstr::Not => w.write_all(&[29]),
        BytecodeInstr::And => w.write_all(&[30]),
        BytecodeInstr::Or => w.write_all(&[31]),
        BytecodeInstr::Jump(off) => {
            w.write_all(&[32])?;
            write_u32(w, off.0 as u32)
        }
        BytecodeInstr::JumpIfFalse(off) => {
            w.write_all(&[33])?;
            write_u32(w, off.0 as u32)
        }
        BytecodeInstr::Return => w.write_all(&[34]),
        BytecodeInstr::Unreachable => w.write_all(&[35]),
        BytecodeInstr::Call(id, argc) => {
            w.write_all(&[36])?;
            write_u32(w, id.0)?;
            w.write_all(&argc.to_le_bytes())
        }
        BytecodeInstr::MakeEnum(enum_id, variant_id, count) => {
            w.write_all(&[37])?;
            write_u32(w, enum_id.0)?;
            write_u32(w, variant_id.0)?;
            w.write_all(&count.to_le_bytes())
        }
        BytecodeInstr::Tag => w.write_all(&[38]),
        BytecodeInstr::Unwrap(variant_id, idx) => {
            w.write_all(&[39])?;
            write_u32(w, variant_id.0)?;
            w.write_all(&idx.to_le_bytes())
        }
        BytecodeInstr::MakeArray(count) => {
            w.write_all(&[40])?;
            w.write_all(&count.to_le_bytes())
        }
        BytecodeInstr::GetIndex => w.write_all(&[41]),
        BytecodeInstr::Slice => w.write_all(&[42]),
        BytecodeInstr::Len => w.write_all(&[43]),
    }
}

// --- Decoding ---

pub fn decode_artifact(bytes: &[u8]) -> Result<BytecodeCacheArtifact, BytecodeSerializeError> {
    let mut cursor = io::Cursor::new(bytes);
    
    // Magic
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(BytecodeSerializeError::InvalidMagic);
    }
    
    // Header
    let format_version = read_u32(&mut cursor)?;
    if format_version != FORMAT_VERSION {
        return Err(BytecodeSerializeError::UnsupportedVersion);
    }
    let compiler_fingerprint = read_string(&mut cursor)?;
    let mut source_hash = [0u8; 32];
    cursor.read_exact(&mut source_hash)?;
    let language_version = read_string(&mut cursor)?;
    let stdlib_abi_version = read_string(&mut cursor)?;
    
    let header = BytecodeCacheHeader {
        format_version,
        compiler_fingerprint,
        source_hash,
        language_version,
        stdlib_abi_version,
    };
    
    // Module
    let mut module = BytecodeModule::new();
    module.version = read_u32(&mut cursor)?;
    
    // Constants
    let const_len = read_u32(&mut cursor)?;
    for _ in 0..const_len {
        let mut tag = [0u8; 1];
        cursor.read_exact(&mut tag)?;
        let c = match tag[0] {
            0 => BytecodeConstant::Unit,
            1 => {
                let mut b = [0u8; 1];
                cursor.read_exact(&mut b)?;
                BytecodeConstant::Bool(b[0] != 0)
            }
            2 => BytecodeConstant::Int64(read_i64(&mut cursor)?),
            3 => BytecodeConstant::UInt64(read_u64(&mut cursor)?),
            4 => BytecodeConstant::Float64(f64::from_bits(read_u64(&mut cursor)?)),
            5 => BytecodeConstant::String(read_string(&mut cursor)?),
            _ => return Err(BytecodeSerializeError::InvalidTag),
        };
        module.constants.push(c);
    }
    
    // Functions
    let fn_len = read_u32(&mut cursor)?;
    for _ in 0..fn_len {
        let name = read_string(&mut cursor)?;
        
        let param_len = read_u32(&mut cursor)?;
        let mut params = Vec::with_capacity(param_len as usize);
        for _ in 0..param_len {
            params.push(read_type(&mut cursor)?);
        }
        
        let ret = read_type(&mut cursor)?;
        
        let mut f = BytecodeFunction::new(name, params, ret);
        
        let local_len = read_u32(&mut cursor)?;
        for _ in 0..local_len {
            f.locals.push(read_type(&mut cursor)?);
        }
        
        f.max_stack = read_u32(&mut cursor)?;
        
        let code_len = read_u32(&mut cursor)?;
        for _ in 0..code_len {
            f.code.push(read_instr(&mut cursor)?);
        }
        
        module.functions.push(f);
    }
    
    // Check trailing bytes
    if cursor.position() < bytes.len() as u64 {
        return Err(BytecodeSerializeError::TrailingBytes);
    }
    
    Ok(BytecodeCacheArtifact { header, module })
}

pub fn decode_and_verify_artifact(bytes: &[u8]) -> Result<BytecodeCacheArtifact, BytecodeSerializeError> {
    let artifact = decode_artifact(bytes)?;
    verify_module(&artifact.module)?;
    Ok(artifact)
}

fn read_u32(r: &mut impl Read) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(r: &mut impl Read) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn read_i64(r: &mut impl Read) -> io::Result<i64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(i64::from_le_bytes(b))
}

fn read_string(r: &mut impl Read) -> Result<String, BytecodeSerializeError> {
    let len = read_u32(r)?;
    let mut b = vec![0u8; len as usize];
    r.read_exact(&mut b)?;
    Ok(String::from_utf8(b)?)
}

fn read_type(r: &mut impl Read) -> Result<BytecodeType, BytecodeSerializeError> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    match b[0] {
        0 => Ok(BytecodeType::Unit),
        1 => Ok(BytecodeType::Bool),
        2 => Ok(BytecodeType::Int64),
        3 => Ok(BytecodeType::UInt64),
        4 => Ok(BytecodeType::Float64),
        5 => Ok(BytecodeType::String),
        6 => Ok(BytecodeType::Any),
        _ => Err(BytecodeSerializeError::InvalidTag),
    }
}

fn read_instr(r: &mut impl Read) -> Result<BytecodeInstr, BytecodeSerializeError> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    match b[0] {
        0 => Ok(BytecodeInstr::ConstUnit),
        1 => {
            let mut v = [0u8; 1];
            r.read_exact(&mut v)?;
            Ok(BytecodeInstr::ConstBool(v[0] != 0))
        }
        2 => Ok(BytecodeInstr::ConstI64(ConstId(read_u32(r)?))),
        3 => Ok(BytecodeInstr::ConstU64(ConstId(read_u32(r)?))),
        4 => Ok(BytecodeInstr::ConstF64(ConstId(read_u32(r)?))),
        5 => Ok(BytecodeInstr::ConstString(ConstId(read_u32(r)?))),
        6 => Ok(BytecodeInstr::LoadLocal(LocalId(read_u32(r)?))),
        7 => Ok(BytecodeInstr::StoreLocal(LocalId(read_u32(r)?))),
        8 => Ok(BytecodeInstr::Pop),
        9 => Ok(BytecodeInstr::Dup),
        10 => Ok(BytecodeInstr::AddI64),
        11 => Ok(BytecodeInstr::SubI64),
        12 => Ok(BytecodeInstr::MulI64),
        13 => Ok(BytecodeInstr::DivI64),
        14 => Ok(BytecodeInstr::AddU64),
        15 => Ok(BytecodeInstr::SubU64),
        16 => Ok(BytecodeInstr::MulU64),
        17 => Ok(BytecodeInstr::DivU64),
        18 => Ok(BytecodeInstr::AddF64),
        19 => Ok(BytecodeInstr::SubF64),
        20 => Ok(BytecodeInstr::MulF64),
        21 => Ok(BytecodeInstr::DivF64),
        22 => Ok(BytecodeInstr::Eq),
        23 => Ok(BytecodeInstr::LtI64),
        24 => Ok(BytecodeInstr::GtI64),
        25 => Ok(BytecodeInstr::LtU64),
        26 => Ok(BytecodeInstr::GtU64),
        27 => Ok(BytecodeInstr::LtF64),
        28 => Ok(BytecodeInstr::GtF64),
        29 => Ok(BytecodeInstr::Not),
        30 => Ok(BytecodeInstr::And),
        31 => Ok(BytecodeInstr::Or),
        32 => Ok(BytecodeInstr::Jump(InstrOffset(read_u32(r)? as usize))),
        33 => Ok(BytecodeInstr::JumpIfFalse(InstrOffset(read_u32(r)? as usize))),
        34 => Ok(BytecodeInstr::Return),
        35 => Ok(BytecodeInstr::Unreachable),
        36 => {
            let func_id = read_u32(r)?;
            let mut argc_bytes = [0u8; 2];
            r.read_exact(&mut argc_bytes)?;
            let argc = u16::from_le_bytes(argc_bytes);
            Ok(BytecodeInstr::Call(FunctionId(func_id), argc))
        }
        37 => {
            let enum_id = ConstId(read_u32(r)?);
            let variant_id = ConstId(read_u32(r)?);
            let mut count_bytes = [0u8; 2];
            r.read_exact(&mut count_bytes)?;
            let count = u16::from_le_bytes(count_bytes);
            Ok(BytecodeInstr::MakeEnum(enum_id, variant_id, count))
        }
        38 => Ok(BytecodeInstr::Tag),
        39 => {
            let variant_id = ConstId(read_u32(r)?);
            let mut idx_bytes = [0u8; 2];
            r.read_exact(&mut idx_bytes)?;
            let idx = u16::from_le_bytes(idx_bytes);
            Ok(BytecodeInstr::Unwrap(variant_id, idx))
        }
        40 => {
            let mut count_bytes = [0u8; 2];
            r.read_exact(&mut count_bytes)?;
            let count = u16::from_le_bytes(count_bytes);
            Ok(BytecodeInstr::MakeArray(count))
        }
        41 => Ok(BytecodeInstr::GetIndex),
        42 => Ok(BytecodeInstr::Slice),
        43 => Ok(BytecodeInstr::Len),
        _ => Err(BytecodeSerializeError::InvalidTag),
    }
}
