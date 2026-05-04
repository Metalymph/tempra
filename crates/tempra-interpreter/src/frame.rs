use tempra_runtime::Value;

/// A single activation record on the call stack.
#[derive(Debug)]
pub struct Frame {
    /// The name of the function this frame belongs to.
    pub function_name: String,
    /// Local operand stack for this frame.
    pub operands: Vec<Value>,
}

/// The interpreter's call stack.
#[derive(Debug)]
pub struct CallStack {
    frames: Vec<Frame>,
}

impl CallStack {
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    /// Push a new frame for the given function.
    pub fn push(&mut self, function_name: String) {
        self.frames.push(Frame {
            function_name,
            operands: Vec::new(),
        });
    }

    /// Pop the top frame, returning it.
    pub fn pop(&mut self) -> Option<Frame> {
        self.frames.pop()
    }

    /// Returns a mutable reference to the current (topmost) frame.
    pub fn current_mut(&mut self) -> Option<&mut Frame> {
        self.frames.last_mut()
    }
}

impl Default for CallStack {
    fn default() -> Self {
        Self::new()
    }
}
