use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::ir::ir::{
    BasicBlock, BlockId, IrBinaryOp, IrFunction, IrInstruction, IrProgram, Terminator, ValueId,
};

pub fn generate(program: &IrProgram) -> Result<String, String> {
    let mut output = String::new();

    writeln!(output, ".intel_syntax noprefix").map_err(|e| e.to_string())?;
    writeln!(output, ".text").map_err(|e| e.to_string())?;

    for function in &program.functions {
        generate_function(&mut output, function)?;
    }

    Ok(output)
}

struct FunctionCodegen<'a> {
    output: &'a mut String,
    function: &'a IrFunction,

    /// Every SSA value gets a stack slot.
    value_slots: HashMap<ValueId, i32>,

    /// Phi copies are emitted on predecessor edges.
    ///
    /// Key:
    ///     predecessor block
    ///
    /// Value:
    ///     copies that must happen when leaving that block
    edge_copies: HashMap<BlockId, Vec<(ValueId, ValueId)>>,

    next_stack_offset: i32,

    /// One extra stack slot used when parallel phi copies form a cycle.
    phi_temp_offset: i32,
}

impl<'a> FunctionCodegen<'a> {
    fn new(output: &'a mut String, function: &'a IrFunction) -> Self {
        Self {
            output,
            function,
            value_slots: HashMap::new(),
            edge_copies: HashMap::new(),
            next_stack_offset: 8,
            phi_temp_offset: 0,
        }
    }

    fn generate(mut self) -> Result<(), String> {
        self.collect_phi_copies();
        self.allocate_value_slots();

        // Reserve one additional stack slot for cycle-breaking phi copies.
        self.phi_temp_offset = self.next_stack_offset;
        self.next_stack_offset += 8;

        writeln!(self.output, ".globl {}", self.function.name)
            .map_err(|e| e.to_string())?;

        writeln!(self.output, "{}:", self.function.name)
            .map_err(|e| e.to_string())?;

        self.emit_prologue()?;

        for block in &self.function.blocks {
            self.emit_block(block)?;
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Phi elimination
    // ---------------------------------------------------------------------

    fn collect_phi_copies(&mut self) {
        for block in &self.function.blocks {
            for instruction in &block.instructions {
                let IrInstruction::Phi {
                    destination,
                    sources,
                    ..
                } = instruction
                else {
                    continue;
                };

                for &(predecessor, source_value) in sources {
                    self.edge_copies
                        .entry(predecessor)
                        .or_default()
                        .push((source_value, *destination));
                }
            }
        }
    }

    fn emit_edge_copies(&mut self, predecessor: BlockId) -> Result<(), String> {
        let Some(copies) = self.edge_copies.get(&predecessor).cloned() else {
            return Ok(());
        };

        self.emit_parallel_copies(&copies)
    }

    /// Emit a set of copies that conceptually happen simultaneously.
    ///
    /// Example:
    ///
    ///     a <- b
    ///     b <- a
    ///
    /// Cannot safely be emitted as:
    ///
    ///     mov a, b
    ///     mov b, a
    ///
    /// because the original value of `a` would be lost.
    ///
    /// We therefore use the temporary stack slot:
    ///
    ///     temp <- a
    ///     a <- b
    ///     b <- temp
    fn emit_parallel_copies(
        &mut self,
        copies: &[(ValueId, ValueId)],
    ) -> Result<(), String> {
        let mut pending: Vec<(ValueId, ValueId)> = copies.to_vec();

        while !pending.is_empty() {
            let mut progress = false;

            // First emit copies whose destination is not used as a source
            // by another pending copy.
            let sources: HashSet<ValueId> =
                pending.iter().map(|(source, _)| *source).collect();

            let mut index = 0;

            while index < pending.len() {
                let (source, destination) = pending[index];

                if source == destination {
                    pending.remove(index);
                    progress = true;
                    continue;
                }

                if !sources.contains(&destination) {
                    self.emit_load(source, "rax")?;
                    self.emit_store(destination, "rax")?;

                    pending.remove(index);
                    progress = true;
                } else {
                    index += 1;
                }
            }

            if progress {
                continue;
            }

            // No safe copy was found, so there is a cycle.
            //
            // Break the cycle by saving one source value in the temporary
            // stack slot.
            let (source, destination) = pending[0];

            self.emit_load(source, "rax")?;

            writeln!(
                self.output,
                "    mov [rbp-{}], rax",
                self.phi_temp_offset
            )
            .map_err(|e| e.to_string())?;

            // Replace the source of this copy with a special temporary
            // representation by removing it and carrying the destination
            // forward.
            pending.remove(0);

            let mut current_destination = destination;

            loop {
                let next_index = pending
                    .iter()
                    .position(|(_, destination)| *destination == source);

                let Some(next_index) = next_index else {
                    break;
                };

                let (next_source, next_destination) = pending.remove(next_index);

                self.emit_load(next_source, "rax")?;
                self.emit_store(current_destination, "rax")?;

                current_destination = next_destination;

                if next_source == source {
                    break;
                }
            }

            // Restore the original value from the temporary slot.
            writeln!(
                self.output,
                "    mov rax, [rbp-{}]",
                self.phi_temp_offset
            )
            .map_err(|e| e.to_string())?;

            self.emit_store(current_destination, "rax")?;
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Stack allocation
    // ---------------------------------------------------------------------

    fn allocate_value_slots(&mut self) {
        for block in &self.function.blocks {
            for instruction in &block.instructions {
                if let Some(value) = instruction_destination(instruction) {
                    self.allocate_value(value);
                }
            }
        }
    }

    fn allocate_value(&mut self, value: ValueId) {
        if self.value_slots.contains_key(&value) {
            return;
        }

        self.value_slots.insert(value, self.next_stack_offset);
        self.next_stack_offset += 8;
    }

    fn stack_size(&self) -> i32 {
        let required = self.next_stack_offset - 8;

        if required <= 0 {
            return 0;
        }

        // Keep the stack 16-byte aligned.
        ((required + 15) / 16) * 16
    }

    fn emit_prologue(&mut self) -> Result<(), String> {
        writeln!(self.output, "    push rbp")
            .map_err(|e| e.to_string())?;

        writeln!(self.output, "    mov rbp, rsp")
            .map_err(|e| e.to_string())?;

        let stack_size = self.stack_size();

        if stack_size > 0 {
            writeln!(self.output, "    sub rsp, {}", stack_size)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Blocks
    // ---------------------------------------------------------------------

    fn emit_block(&mut self, block: &BasicBlock) -> Result<(), String> {
        writeln!(
            self.output,
            ".L{}_{}:",
            self.function.name,
            block.id.0
        )
        .map_err(|e| e.to_string())?;

        for instruction in &block.instructions {
            // Phi instructions have already been converted into edge copies.
            if matches!(instruction, IrInstruction::Phi { .. }) {
                continue;
            }

            self.emit_instruction(instruction)?;
        }

        // Phi copies belong to the predecessor edge, so they must happen
        // after ordinary instructions in this block but before its
        // terminator transfers control.
        self.emit_edge_copies(block.id)?;

        self.emit_terminator(&block.terminator)?;

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------

    fn emit_instruction(
        &mut self,
        instruction: &IrInstruction,
    ) -> Result<(), String> {
        match instruction {
            IrInstruction::Parameter {
                destination,
                local: _,
            } => {
                return Err(format!(
                    "parameter code generation is not implemented yet for value {:?}",
                    destination
                ));
            }

            IrInstruction::ConstInt {
                destination,
                value,
            } => {
                self.emit_load_immediate("rax", *value)?;
                self.emit_store(*destination, "rax")?;
            }

            IrInstruction::ConstString {
                destination,
                value,
            } => {
                return Err(format!(
                    "string code generation is not implemented yet for {:?} = {:?}",
                    destination,
                    value
                ));
            }

            IrInstruction::ConstBool {
                destination,
                value,
            } => {
                let value = if *value { 1 } else { 0 };

                writeln!(self.output, "    mov rax, {}", value)
                    .map_err(|e| e.to_string())?;

                self.emit_store(*destination, "rax")?;
            }

            IrInstruction::LoadLocal {
                destination,
                local: _,
            } => {
                return Err(format!(
                    "LoadLocal should have been removed by SSA construction: {:?}",
                    destination
                ));
            }

            IrInstruction::StoreLocal { local, value } => {
                return Err(format!(
                    "StoreLocal should have been removed by SSA construction: local {:?}, value {:?}",
                    local,
                    value
                ));
            }

            IrInstruction::Binary {
                destination,
                op,
                left,
                right,
            } => {
                self.emit_load(*left, "rax")?;
                self.emit_load(*right, "rcx")?;

                match op {
                    IrBinaryOp::Add => {
                        writeln!(self.output, "    add rax, rcx")
                            .map_err(|e| e.to_string())?;
                    }

                    IrBinaryOp::Subtract => {
                        writeln!(self.output, "    sub rax, rcx")
                            .map_err(|e| e.to_string())?;
                    }

                    IrBinaryOp::Multiply => {
                        writeln!(self.output, "    imul rax, rcx")
                            .map_err(|e| e.to_string())?;
                    }

                    IrBinaryOp::Divide => {
                        writeln!(self.output, "    cqo")
                            .map_err(|e| e.to_string())?;

                        writeln!(self.output, "    idiv rcx")
                            .map_err(|e| e.to_string())?;
                    }

                    IrBinaryOp::Equal => {
                        writeln!(self.output, "    cmp rax, rcx")
                            .map_err(|e| e.to_string())?;

                        writeln!(self.output, "    sete al")
                            .map_err(|e| e.to_string())?;

                        writeln!(self.output, "    movzx rax, al")
                            .map_err(|e| e.to_string())?;
                    }
                }

                self.emit_store(*destination, "rax")?;
            }

            IrInstruction::Call {
                destination,
                function,
                arguments,
            } => {
                return Err(format!(
                    "function calls are not implemented yet: function {:?}, destination {:?}, {} arguments",
                    function,
                    destination,
                    arguments.len()
                ));
            }

            IrInstruction::Phi { .. } => {
                // Phi nodes are handled by emit_edge_copies().
            }
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Terminators
    // ---------------------------------------------------------------------

    fn emit_terminator(
        &mut self,
        terminator: &Terminator,
    ) -> Result<(), String> {
        match terminator {
            Terminator::Jump(block) => {
                writeln!(
                    self.output,
                    "    jmp {}",
                    self.block_label(*block)
                )
                .map_err(|e| e.to_string())?;
            }

            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => {
                self.emit_load(*condition, "rax")?;

                writeln!(self.output, "    cmp rax, 0")
                    .map_err(|e| e.to_string())?;

                writeln!(
                    self.output,
                    "    jne {}",
                    self.block_label(*then_block)
                )
                .map_err(|e| e.to_string())?;

                writeln!(
                    self.output,
                    "    jmp {}",
                    self.block_label(*else_block)
                )
                .map_err(|e| e.to_string())?;
            }

            Terminator::Return { value } => {
                if let Some(value) = value {
                    self.emit_load(*value, "rax")?;
                }

                writeln!(self.output, "    leave")
                    .map_err(|e| e.to_string())?;

                writeln!(self.output, "    ret")
                    .map_err(|e| e.to_string())?;
            }

            Terminator::Unreachable => {
                writeln!(self.output, "    ud2")
                    .map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Values
    // ---------------------------------------------------------------------

    fn emit_load(
        &mut self,
        value: ValueId,
        register: &str,
    ) -> Result<(), String> {
        let offset = self
            .value_slots
            .get(&value)
            .ok_or_else(|| {
                format!(
                    "no stack slot allocated for SSA value {:?}",
                    value
                )
            })?;

        writeln!(
            self.output,
            "    mov {}, [rbp-{}]",
            register,
            offset
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn emit_store(
        &mut self,
        value: ValueId,
        register: &str,
    ) -> Result<(), String> {
        let offset = self
            .value_slots
            .get(&value)
            .ok_or_else(|| {
                format!(
                    "no stack slot allocated for SSA value {:?}",
                    value
                )
            })?;

        writeln!(
            self.output,
            "    mov [rbp-{}], {}",
            offset,
            register
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn emit_load_immediate(
        &mut self,
        register: &str,
        value: i64,
    ) -> Result<(), String> {
        writeln!(
            self.output,
            "    mov {}, {}",
            register,
            value
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn block_label(&self, block: BlockId) -> String {
        format!(".L{}_{}", self.function.name, block.0)
    }
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

fn instruction_destination(
    instruction: &IrInstruction,
) -> Option<ValueId> {
    match instruction {
        IrInstruction::Parameter { destination, .. } => {
            Some(*destination)
        }

        IrInstruction::ConstInt { destination, .. } => {
            Some(*destination)
        }

        IrInstruction::ConstString { destination, .. } => {
            Some(*destination)
        }

        IrInstruction::ConstBool { destination, .. } => {
            Some(*destination)
        }

        IrInstruction::LoadLocal { destination, .. } => {
            Some(*destination)
        }

        IrInstruction::StoreLocal { .. } => None,

        IrInstruction::Binary { destination, .. } => {
            Some(*destination)
        }

        IrInstruction::Call { destination, .. } => {
            *destination
        }

        IrInstruction::Phi { destination, .. } => {
            Some(*destination)
        }
    }
}

fn generate_function(
    output: &mut String,
    function: &IrFunction,
) -> Result<(), String> {
    FunctionCodegen::new(output, function).generate()
}