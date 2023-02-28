use indicatif::*;
use llir::values::*;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;
use std::time::SystemTime;

use crate::call_graph::*;
use crate::semantics::{rced::*, *};
use crate::slicer::*;
use crate::utils::*;

use super::*;

pub struct SymbolicExecutionContext<'a, O>
where
    O: SymbolicExecutionOptions,
{
    pub options: &'a O,
}

impl<'a, 'ctx, O> SymbolicExecutionContext<'a, O>
where
    O: SymbolicExecutionOptions,
{
    pub fn new(options: &'a O) -> Self {
        Self { options }
    }

    pub fn execute_function(
        &self,
        instr_node_id: usize,
        instr: CallInstruction<'ctx>,
        func: Function<'ctx>,
        args: Vec<Rc<Value>>,
        state: &mut State<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        match func.first_block() {
            Some(block) => {
                // Init the execution memory state.
                let stack_frame = StackFrame {
                    function: func,
                    instr: Some((instr_node_id, instr)),
                    memory: LocalMemory::new(),
                    arguments: args,
                };
                state.stack.push(stack_frame);
                self.execute_block(block, state)
            }
            None => panic!("The executed function is empty"),
        }
    }

    pub fn execute_block(&self, block: Block<'ctx>, state: &mut State<'ctx>) -> Option<Instruction<'ctx>> {
        match state.prev_block {
            Some(prev_block) => {
                state.block_trace_iter.visit_block(prev_block, block, true);
            }
            _ => {}
        }
        block.first_instruction()
    }

    pub fn execute_instr(
        &self,
        instr: Option<Instruction<'ctx>>,
        state: &mut State<'ctx>,
        env: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        if state.trace.len() > self.options.max_node_per_trace() {
            state.finish_state = FinishState::ExceedingMaxTraceLength;
            None
        } else if state.has_timeouted(self.options.max_timeout()) {
            state.finish_state = FinishState::Timeout;
            None
        } else {
            match instr {
                Some(instr) => {
                    use Instruction::*;
                    match instr {
                        Return(ret) => self.transfer_ret_instr(ret, state, env),
                        Branch(br) => self.transfer_br_instr(br, state, env),
                        Switch(swi) => self.transfer_switch_instr(swi, state, env),
                        Call(call) => self.transfer_call_instr(call, state, env),
                        Store(st) => self.transfer_store_instr(st, state, env),
                        ICmp(icmp) => self.transfer_icmp_instr(icmp, state, env),
                        Load(ld) => self.transfer_load_instr(ld, state, env),
                        Phi(phi) => self.transfer_phi_instr(phi, state, env),
                        GetElementPtr(gep) => self.transfer_gep_instr(gep, state, env),
                        Unreachable(unr) => self.transfer_unreachable_instr(unr, state, env),
                        Binary(bin) => self.transfer_binary_instr(bin, state, env),
                        Unary(una) => self.transfer_unary_instr(una, state, env),
                        _ => instr.next_instruction(),
                    }
                }
                None => None,
            }
        }
    }

    pub fn eval_constant_value(&self, state: &mut State<'ctx>, constant: Constant<'ctx>) -> Rc<Value> {
        match constant {
            Constant::Int(i) => Rc::new(Value::Int(i.sext_value())),
            Constant::Null(_) => Rc::new(Value::Null),
            Constant::Float(_) | Constant::Struct(_) | Constant::Array(_) | Constant::Vector(_) => {
                Rc::new(Value::ConstSym(state.new_symbol_id()))
            }
            Constant::Global(glob) => Rc::new(Value::Glob(glob.name())),
            Constant::Function(func) => Rc::new(Value::Func(func.simp_name())),
            Constant::ConstExpr(ce) => match ce {
                ConstExpr::Binary(b) => {
                    let op = b.opcode();
                    let op0 = self.eval_constant_value(state, b.op0());
                    let op1 = self.eval_constant_value(state, b.op1());
                    Rc::new(Value::Bin { op, op0, op1 })
                }
                ConstExpr::Unary(u) => self.eval_constant_value(state, u.op0()),
                ConstExpr::GetElementPtr(g) => {
                    let loc = self.eval_constant_value(state, g.location());
                    let indices = g
                        .indices()
                        .into_iter()
                        .map(|i| self.eval_constant_value(state, i))
                        .collect();
                    Rc::new(Value::GEP { loc, indices })
                }
                _ => Rc::new(Value::Unknown),
            },
            _ => Rc::new(Value::Unknown),
        }
    }

    pub fn eval_operand_value(&self, state: &mut State<'ctx>, operand: Operand<'ctx>) -> Rc<Value> {
        match operand {
            Operand::Instruction(instr) => {
                if state.stack.top().memory.contains_key(&instr) {
                    state.stack.top().memory[&instr].clone()
                } else {
                    match instr {
                        Instruction::Alloca(_) => {
                            let alloca_id = state.new_alloca_id();
                            let value = Rc::new(Value::Alloc(alloca_id));
                            state.stack.top_mut().memory.insert(instr, value.clone());
                            value
                        }
                        _ => Rc::new(Value::Unknown),
                    }
                }
            }
            Operand::Argument(arg) => {
                if state.stack.top().arguments.len() > arg.index() {
                    state.stack.top().arguments[arg.index()].clone()
                } else {
                    Rc::new(Value::Unknown)
                }
            }
            Operand::Constant(cons) => self.eval_constant_value(state, cons),
            Operand::InlineAsm(_) => Rc::new(Value::Asm),
            _ => Rc::new(Value::Unknown),
        }
    }

    // Set the memory value as symbol when it is a argument of a call which will not be stepped in.
    pub fn replace_value_as_sym(&self, state: &mut State<'ctx>, operand: Operand<'ctx>) -> bool {
        match operand {
            Operand::Instruction(instr) => match instr {
                Instruction::Unary(unary_instr) => {
                    return self.replace_value_as_sym(state, unary_instr.op0());
                }
                Instruction::Alloca(_) | Instruction::GetElementPtr(_) => {
                    let symbol_id = state.new_symbol_id();
                    let loc = self.eval_operand_value(state, operand);
                    let res = self.load_from_memory(state, loc.clone());
                    match *res {
                        Value::GlobSym(_) => {
                            let new_value = Rc::new(Value::GlobSym(symbol_id));
                            state.memory.insert(loc, new_value);
                        }
                        _ => {
                            let new_value = Rc::new(Value::Sym(symbol_id));
                            state.memory.insert(loc, new_value);
                        }
                    }
                    return true;
                }
                _ => {}
            },
            _ => {}
        }
        true
    }

    pub fn load_from_base_loc(&self, state: &mut State<'ctx>, base_loc: Rc<Value>, location: Rc<Value>) -> Rc<Value> {
        let value;
        match &*base_loc {
            Value::GEP { loc, indices: _ } => {
                // Further validate the type of base_loc to assign a proper symbol type
                return self.load_from_base_loc(state, loc.clone(), location);
            }
            Value::Glob(_) | Value::Arg(_) => {
                let symbol_id = state.new_symbol_id();
                value = Rc::new(Value::GlobSym(symbol_id));
                state.memory.insert(location, value.clone());
            }
            _ => {
                let symbol_id = state.new_symbol_id();
                value = Rc::new(Value::Sym(symbol_id));
                state.memory.insert(location, value.clone());
            }
        };
        value
    }

    pub fn load_from_memory(&self, state: &mut State<'ctx>, location: Rc<Value>) -> Rc<Value> {
        match &*location {
            Value::Unknown => Rc::new(Value::Unknown),
            Value::GEP { loc, indices: _ } => match state.memory.get(&location) {
                Some(value) => value.clone(),
                None => {
                    // Further validate the type of base_loc to assign a proper symbol type
                    self.load_from_base_loc(state, loc.clone(), location)
                }
            },
            Value::Glob(_) | Value::Arg(_) => match state.memory.get(&location) {
                Some(value) => value.clone(),
                None => {
                    let symbol_id = state.new_symbol_id();
                    let value = Rc::new(Value::GlobSym(symbol_id));
                    state.memory.insert(location, value.clone());
                    value
                }
            },
            _ => match state.memory.get(&location) {
                Some(value) => value.clone(),
                None => {
                    let symbol_id = state.new_symbol_id();
                    let value = Rc::new(Value::Sym(symbol_id));
                    state.memory.insert(location, value.clone());
                    value
                }
            },
        }
    }

    pub fn transfer_ret_instr(
        &self,
        instr: ReturnInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        // First evaluate the return operand. There might not be one
        let val = instr.op().map(|val| self.eval_operand_value(state, val));
        state.trace.push(TraceNode {
            instr: instr.as_instruction(),
            semantics: Semantics::Ret { op: val.clone() },
            result: None,
        });

        // Then we peek the stack frame
        let stack_frame = state.stack.pop().unwrap(); // There has to be a stack on the top
        match stack_frame.instr {
            Some((node_id, call_site)) => {
                let call_site_frame = state.stack.top_mut(); // If call site exists then there must be a stack top
                if let Some(op0) = val {
                    if stack_frame.function.get_function_type().has_return_type() {
                        state.trace[node_id].result = Some(op0.clone());
                        call_site_frame.memory.insert(call_site.as_instruction(), op0);
                    }
                }
                if state.in_relevant_method {
                    state.in_relevant_method = false;
                }
                call_site.next_instruction()
            }

            // If no call site then we are in the entry function. We will end the execution
            None => {
                state.finish_state = FinishState::ProperlyReturned;
                None
            }
        }
    }

    pub fn transfer_unconditional_br_instr(
        &self,
        instr: UnconditionalBranchInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        // Set previous block
        let curr_blk = instr.parent_block();
        state.prev_block = Some(curr_blk);
        if instr.is_loop_jump().unwrap_or(false) {
            state.loop_depth -= 1;
        }
        self.execute_block(instr.destination(), state)
    }

    pub fn transfer_conditional_br_instr(
        &self,
        instr: ConditionalBranchInstruction<'ctx>,
        state: &mut State<'ctx>,
        env: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        // Set previous block
        let curr_blk = instr.parent_block();
        state.prev_block = Some(curr_blk);
        // Get Branch Direction
        let then_br = BranchDirection {
            from: curr_blk,
            to: instr.then_block(),
        };
        let else_br = BranchDirection {
            from: curr_blk,
            to: instr.else_block(),
        };
        // Check condition
        let cond = self.eval_operand_value(state, instr.condition().into());
        let comparison = cond.as_comparison();
        let is_loop_blk = curr_blk.is_loop_entry_block();
        if is_loop_blk {
            state.loop_depth += 1;
        }

        // Check whether to the branch condition can be satisfied
        let mut can_visit_then = true;
        let mut can_visit_else = true;
        if let Some(comparison) = comparison.clone() {
            can_visit_then = comparison.evaluate_cond(true) || env.is_rough_mode();
            can_visit_else = comparison.evaluate_cond(false) || env.is_rough_mode();
        }
        let visited_then = state.visited_branch.contains(&then_br);
        let visited_else = state.visited_branch.contains(&else_br);
        let need_visit_then = state.block_trace_iter.visit_block(curr_blk, instr.then_block(), false);
        let need_visit_else = state.block_trace_iter.visit_block(curr_blk, instr.else_block(), false);

        if can_visit_then && (need_visit_then || (!need_visit_else && !visited_then) || state.in_relevant_method) {
            // First add else branch into work
            if !need_visit_then && can_visit_else && !visited_else && self.can_add_work(env.work_list.len()) {
                let mut else_state = state.clone();
                // Add constraint & Update state
                if let Some(comparison) = comparison.clone() {
                    else_state.add_constraint(comparison, false);
                }
                else_state.visited_branch.insert(else_br);
                else_state.trace.push(TraceNode {
                    instr: instr.as_instruction(),
                    semantics: Semantics::CondBr {
                        cond: cond.clone(),
                        br: Branch::Else,
                    },
                    result: None,
                });
                // Generate work
                let else_work = Work::new(instr.else_block(), else_state);
                env.add_work(else_work);
            }
            // Then execute the then branch
            if let Some(comparison) = comparison.clone() {
                state.add_constraint(comparison, true);
            }
            state.visited_branch.insert(then_br);
            state.trace.push(TraceNode {
                instr: instr.as_instruction(),
                semantics: Semantics::CondBr { cond, br: Branch::Then },
                result: None,
            });
            self.execute_block(instr.then_block(), state)
        } else if can_visit_else && !need_visit_then && !visited_else {
            if !need_visit_else && can_visit_then && !visited_then && self.can_add_work(env.work_list.len()) {
                let mut then_state = state.clone();
                // Add constraint & Update state
                if let Some(comparison) = comparison.clone() {
                    then_state.add_constraint(comparison, true);
                }
                then_state.visited_branch.insert(then_br);
                then_state.trace.push(TraceNode {
                    instr: instr.as_instruction(),
                    semantics: Semantics::CondBr {
                        cond: cond.clone(),
                        br: Branch::Then,
                    },
                    result: None,
                });
                // Generate work
                let then_work = Work::new(instr.then_block(), then_state);
                env.add_work(then_work);
            }
            // Execute the else branch
            if let Some(comparison) = comparison.clone() {
                state.add_constraint(comparison, false);
            }
            state.visited_branch.insert(else_br);
            state.trace.push(TraceNode {
                instr: instr.as_instruction(),
                semantics: Semantics::CondBr { cond, br: Branch::Else },
                result: None,
            });
            self.execute_block(instr.else_block(), state)
        } else if !visited_then && can_visit_then && need_visit_else && !need_visit_then {
            // Correct the guiding block traces
            if state.block_trace_iter.correct_blk_paths(instr.then_block()) {
                if let Some(comparison) = comparison.clone() {
                    state.add_constraint(comparison, true);
                }
                state.visited_branch.insert(then_br);
                state.trace.push(TraceNode {
                    instr: instr.as_instruction(),
                    semantics: Semantics::CondBr { cond, br: Branch::Then },
                    result: None,
                });
                self.execute_block(instr.then_block(), state)
            } else {
                state.finish_state = FinishState::BranchExplored;
                None
            }
        } else if !visited_else && can_visit_else && need_visit_then && !need_visit_else {
            // Correct the guiding block traces
            if state.block_trace_iter.correct_blk_paths(instr.else_block()) {
                if let Some(comparison) = comparison.clone() {
                    state.add_constraint(comparison, false);
                }
                state.visited_branch.insert(else_br);
                state.trace.push(TraceNode {
                    instr: instr.as_instruction(),
                    semantics: Semantics::CondBr { cond, br: Branch::Else },
                    result: None,
                });
                self.execute_block(instr.else_block(), state)
            } else {
                state.finish_state = FinishState::BranchExplored;
                None
            }
        } else {
            // If both then and else are visited, stop the execution with BranchExplored
            state.finish_state = FinishState::BranchExplored;
            None
        }
    }

    pub fn transfer_br_instr(
        &self,
        instr: BranchInstruction<'ctx>,
        state: &mut State<'ctx>,
        env: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        match instr {
            BranchInstruction::Conditional(cb) => self.transfer_conditional_br_instr(cb, state, env),
            BranchInstruction::Unconditional(ub) => self.transfer_unconditional_br_instr(ub, state, env),
        }
    }

    pub fn transfer_switch_instr(
        &self,
        instr: SwitchInstruction<'ctx>,
        state: &mut State<'ctx>,
        env: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        // Set previous block
        let curr_blk = instr.parent_block();
        state.prev_block = Some(curr_blk);
        let cond = self.eval_operand_value(state, instr.condition().into());
        let branches = instr
            .cases()
            .iter()
            .map(|case| BranchDirection {
                from: curr_blk,
                to: case.destination,
            })
            .collect::<Vec<_>>();
        let default_br = BranchDirection {
            from: curr_blk,
            to: instr.default_destination(),
        };
        let node = TraceNode {
            instr: instr.as_instruction(),
            semantics: Semantics::Switch { cond },
            result: None,
        };
        state.trace.push(node);

        if state
            .block_trace_iter
            .visit_block(curr_blk, instr.default_destination(), false)
        {
            return self.execute_block(instr.default_destination(), state);
        }

        // Insert branches as work if not visited
        for bd in &branches {
            if state.block_trace_iter.visit_block(curr_blk, bd.to, false) {
                state.visited_branch.insert(*bd);
                return self.execute_block(bd.to, state);
            }
        }

        for bd in branches {
            if !state.visited_branch.contains(&bd) && self.can_add_work(env.work_list.len()) {
                let mut br_state = state.clone();
                br_state.visited_branch.insert(bd);
                let br_work = Work::new(bd.to, br_state);
                env.add_work(br_work);
            }
        }

        // Execute default branch
        if !state.visited_branch.contains(&default_br) {
            state.visited_branch.insert(default_br);
            self.execute_block(instr.default_destination(), state)
        } else {
            state.finish_state = FinishState::BranchExplored;
            None
        }
    }

    pub fn transfer_call_instr(
        &self,
        instr: CallInstruction<'ctx>,
        state: &mut State<'ctx>,
        env: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        // If is intrinsic call, skip the instruction
        if instr.is_intrinsic_call() {
            instr.next_instruction()
        } else {
            // Check if stepping in the function, and get the function Value and also
            // maybe function reference
            let (step_in, func_value, func) = match instr.callee_function() {
                Some(func) => {
                    let step_in = !func.is_declaration_only()
                        && func != env.slice.callee
                        && !state.stack.has_function(func)
                        // Whether to step in if the slice depth is 0
                        && (env.slice.entry != env.slice.caller || self.options.step_in_anytime())
                        && env.slice.functions.contains(&(func, instr));
                    (step_in, Rc::new(Value::Func(func.simp_name())), Some(func))
                }
                None => {
                    if instr.is_inline_asm_call() {
                        (false, Rc::new(Value::Asm), None)
                    } else {
                        (false, Rc::new(Value::FuncPtr), None)
                    }
                }
            };

            // Evaluate the arguments
            let args = instr
                .arguments()
                .into_iter()
                .map(|v| self.eval_operand_value(state, v))
                .collect::<Vec<_>>();

            // Cache the node id for this call
            let node_id = state.trace.len();

            // Generate a semantics and push to the trace
            let semantics = Semantics::Call {
                func: func_value.clone(),
                args: args.clone(),
            };
            let node = TraceNode {
                instr: instr.as_instruction(),
                semantics,
                result: None,
            };
            state.trace.push(node);

            // Update the target_node in state if the target is now visited
            if instr == env.slice.instr && state.target_node.is_none() {
                state.target_node = Some(node_id);
            }

            // Update status of block traces for future direction
            state.block_trace_iter.visit_call(instr);
            // Check if we need to add a work to step in the function
            if step_in && !state.in_relevant_method {
                state.in_relevant_method = true;
                return self.execute_function(node_id, instr, func.unwrap(), args.clone(), state);
            }
            // Directly step in if the function in the call chain
            let callchain_len = env.slice.call_chain.succ.len();
            if callchain_len > 1
                && func != None
                && env.slice.call_chain.succ[..callchain_len - 1].contains(&(instr, func.unwrap()))
            {
                self.execute_function(node_id, instr, func.unwrap(), args.clone(), state)
            } else {
                // Only add call result if the callee function has return type
                if instr.callee_function_type().has_return_type() {
                    // Create a function call result with a call_id associated(context-sensitive)
                    let call_id = env.new_call_id();
                    let result = Rc::new(Value::Call {
                        id: call_id,
                        func: func_value.clone(),
                        args: args.clone(),
                    });

                    // Update the result stored in the trace
                    state.trace[node_id].result = Some(result.clone());
                    // Insert a result to the stack frame memory
                    state.stack.top_mut().memory.insert(instr.as_instruction(), result);
                }
                // Update the arguments which refers to a memory
                for value in instr.arguments() {
                    self.replace_value_as_sym(state, value);
                }
                instr.next_instruction()
            }
        }
    }

    pub fn transfer_store_instr(
        &self,
        instr: StoreInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        let loc = self.eval_operand_value(state, instr.location());
        let val = self.eval_operand_value(state, instr.value());
        match *val {
            Value::Sym(_) => {
                let ori_val = self.load_from_memory(state, loc.clone());
                match *ori_val {
                    // Keep the original global symbol type
                    Value::GlobSym(_) => {
                        let symbol_id = state.new_symbol_id();
                        let new_value = Rc::new(Value::GlobSym(symbol_id));
                        state.memory.insert(loc.clone(), new_value);
                    }
                    _ => {
                        state.memory.insert(loc.clone(), val.clone());
                    }
                }
            }
            _ => {
                state.memory.insert(loc.clone(), val.clone());
            }
        }
        let node = TraceNode {
            instr: instr.as_instruction(),
            semantics: Semantics::Store { loc, val },
            result: None,
        };
        state.trace.push(node);
        instr.next_instruction()
    }

    pub fn transfer_load_instr(
        &self,
        instr: LoadInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        let loc = self.eval_operand_value(state, instr.location());
        let res = self.load_from_memory(state, loc.clone());
        let node = TraceNode {
            instr: instr.as_instruction(),
            semantics: Semantics::Load { loc },
            result: Some(res.clone()),
        };
        state.trace.push(node);
        state.stack.top_mut().memory.insert(instr.as_instruction(), res);
        instr.next_instruction()
    }

    pub fn transfer_icmp_instr(
        &self,
        instr: ICmpInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        let pred = instr.predicate(); // ICMP must have a predicate
        let op0 = self.eval_operand_value(state, instr.op0());
        let op1 = self.eval_operand_value(state, instr.op1());
        let res = Rc::new(Value::ICmp {
            pred,
            op0: op0.clone(),
            op1: op1.clone(),
        });
        let semantics = Semantics::ICmp { pred, op0, op1 };
        let node = TraceNode {
            instr: instr.as_instruction(),
            semantics,
            result: Some(res.clone()),
        };
        state.trace.push(node);
        state.stack.top_mut().memory.insert(instr.as_instruction(), res);
        instr.next_instruction()
    }

    pub fn transfer_phi_instr(
        &self,
        instr: PhiInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        let prev_blk = state.prev_block.unwrap();
        let incoming_val = instr
            .incomings()
            .iter()
            .find(|incoming| incoming.block == prev_blk) // confirm to take which def
            .unwrap()
            .value;
        let res = self.eval_operand_value(state, incoming_val);
        state.stack.top_mut().memory.insert(instr.as_instruction(), res);
        instr.next_instruction()
    }

    pub fn transfer_gep_instr(
        &self,
        instr: GetElementPtrInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        let loc = self.eval_operand_value(state, instr.location());
        let indices = instr
            .indices()
            .iter()
            .map(|index| self.eval_operand_value(state, *index))
            .collect::<Vec<_>>();
        let res = Rc::new(Value::GEP {
            loc: loc.clone(),
            indices: indices.clone(),
        });
        let node = TraceNode {
            instr: instr.as_instruction(),
            semantics: Semantics::GEP {
                loc: loc.clone(),
                indices,
            },
            result: Some(res.clone()),
        };
        state.trace.push(node);
        state.stack.top_mut().memory.insert(instr.as_instruction(), res);
        instr.next_instruction()
    }

    pub fn transfer_binary_instr(
        &self,
        instr: BinaryInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        let op = instr.binary_opcode();
        let v0 = self.eval_operand_value(state, instr.op0());
        let v1 = self.eval_operand_value(state, instr.op1());
        let res = Rc::new(Value::Bin {
            op,
            op0: v0.clone(),
            op1: v1.clone(),
        });
        let node = TraceNode {
            instr: instr.as_instruction(),
            semantics: Semantics::Bin { op, op0: v0, op1: v1 },
            result: Some(res.clone()),
        };
        state.trace.push(node);
        // If in loop, then conservatively set the binary op as a fresh symbol,
        // since we only iterate it in once.
        if state.loop_depth > 0 {
            let symbol_id = state.new_symbol_id();
            let new_value = Rc::new(Value::Sym(symbol_id));
            state.stack.top_mut().memory.insert(instr.as_instruction(), new_value);
        } else {
            state.stack.top_mut().memory.insert(instr.as_instruction(), res);
        }
        instr.next_instruction()
    }

    pub fn transfer_unary_instr(
        &self,
        instr: UnaryInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        let op = instr.unary_opcode();
        let op0 = self.eval_operand_value(state, instr.op0());
        let node = TraceNode {
            instr: instr.as_instruction(),
            semantics: Semantics::Una { op, op0: op0.clone() },
            result: Some(op0.clone()),
        };
        state.trace.push(node);
        state.stack.top_mut().memory.insert(instr.as_instruction(), op0);
        instr.next_instruction()
    }

    pub fn transfer_unreachable_instr(
        &self,
        _: UnreachableInstruction<'ctx>,
        state: &mut State<'ctx>,
        _: &mut Environment<'ctx>,
    ) -> Option<Instruction<'ctx>> {
        state.finish_state = FinishState::Unreachable;
        None
    }

    pub fn can_add_work(&self, curr_work_num: usize) -> bool {
        if curr_work_num > self.options.max_explored_trace_per_slice() / 2 {
            return false;
        }
        true
    }

    pub fn continue_execution(&self, metadata: &MetaData) -> bool {
        metadata.explored_trace_count < self.options.max_explored_trace_per_slice()
            && metadata.proper_trace_count < self.options.max_trace_per_slice()
            && metadata.timeout_trace_count < 3
    }

    pub fn finish_execution(
        &self,
        state: State<'ctx>,
        slice_id: usize,
        metadata: &mut MetaData,
        env: &mut Environment<'ctx>,
    ) {
        match state.finish_state {
            FinishState::ProperlyReturned => {
                match state.target_node {
                    Some(target_id) => {
                        // Generate the trace for output
                        let trace = TraceWithTarget::new(state.trace, target_id);

                        // Check block trace duplication
                        let block_trace = trace.block_trace();
                        if env.is_rough_mode() || !env.has_duplicate(&block_trace) {
                            // Add block trace into environment
                            env.add_block_trace(block_trace);

                            // Check path satisfaction
                            if env.is_rough_mode() || state.constraints.sat(state.symbol_id) {
                                // Need store
                                let trace_id = metadata.proper_trace_count;
                                let path = self.options.trace_target_slice_file_path(
                                    env.slice.target_function_name().as_str(),
                                    slice_id,
                                    trace_id,
                                );

                                // Dump the json
                                let json_value = trace.to_json();
                                match json_value {
                                    Ok(json_value) => {
                                        dump_json(&json_value, path).expect("Cannot dump json");
                                        // Increase the count in metadata
                                        metadata.incr_proper();
                                    }
                                    Err(_) => {
                                        metadata.incr_timeout();
                                    }
                                }
                            } else {
                                metadata.incr_path_unsat()
                            }
                        } else {
                            metadata.incr_duplicated()
                        }
                    }
                    // This should be rarely appear
                    None => metadata.incr_no_target(),
                }
            }
            FinishState::BranchExplored => metadata.incr_branch_explored(),
            FinishState::ExceedingMaxTraceLength => metadata.incr_exceeding_length(),
            FinishState::Unreachable => metadata.incr_unreachable(),
            FinishState::Timeout => metadata.incr_timeout(),
        }
    }

    pub fn execute_block_state(&self, block: Block<'ctx>, state: &mut State<'ctx>, env: &mut Environment<'ctx>) {
        let mut curr_instr = self.execute_block(block, state);
        while curr_instr.is_some() {
            curr_instr = self.execute_instr(curr_instr, state, env);
        }
    }

    pub fn execute_slice(&self, slice: Slice<'ctx>, slice_id: usize) -> MetaData {
        let mut metadata = MetaData::new();
        let mut env = Environment::new(&slice, self.options.is_rough());

        let block_traces = slice.block_traces(self.options.max_trace_per_slice(), self.options.not_random_scheduling());

        // Init works according to guiding block traces
        for block_trace in &block_traces {
            let work = Work::entry_with_block_trace(
                &slice,
                block_trace.to_vec(),
                self.options.max_trace_per_slice(),
                self.options.not_random_scheduling(),
            );
            env.add_work(work);
        }

        // Iterate till no more work to be done or should end execution
        while env.has_work() && self.continue_execution(&metadata) {
            // Randomly schedule works by default since there is insufficient data to direct
            let mut work = env.pop_work(self.options.not_random_scheduling());

            // Set the start_time for Timeout sanitizer
            work.state.start_time = SystemTime::now();

            // Start the execution by iterating through instructions
            self.execute_block_state(work.block, &mut work.state, &mut env);

            // Finish the instruction and settle down the states
            self.finish_execution(work.state, slice_id, &mut metadata, &mut env);

            // Conservatively to capture the basic patterns of the slice with complicated paths
            if metadata.proper_trace_count == 0
                && (metadata.explored_trace_count == self.options.max_explored_trace_per_slice() - 1
                    || !env.has_work()) {
                let rough_work = Work::entry_with_block_trace(&slice, block_traces[0].clone(), 0, false);
                env.add_work(rough_work);
                env.change_to_rough();
            }
        }
        metadata
    }

    fn initialize_traces_function_slice_folder(&self, func_name: &String, slice_id: usize) -> Result<(), String> {
        let path = self.options.trace_target_slice_dir(func_name.as_str(), slice_id);
        fs::create_dir_all(path).map_err(|_| "Cannot create trace function slice folder".to_string())
    }

    pub fn execute_target_slices(
        &self,
        target_name: &String,
        slice_id_offset: usize,
        slices: Vec<Slice<'ctx>>,
    ) -> MetaData {
        let num_slices = slices.len();
        let style = ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:50.cyan/white} {pos:>5}/{len:5} {percent}% {msg}")
            .progress_chars("##-");
        let pb = ProgressBar::new(num_slices as u64).with_style(style);
        pb.set_message(target_name);

        // Execute each slice in serial
        if self.options.use_serial() {
            println!("Processing function: {} with {} slices", target_name, num_slices);
            slices.into_iter().progress_with(pb).enumerate().fold(
                MetaData::new(),
                |meta: MetaData, (id, slice): (usize, Slice<'ctx>)| {
                    let slice_id = slice_id_offset + id;
                    self.initialize_traces_function_slice_folder(target_name, slice_id)
                        .unwrap();
                    if slice.instr.debug_loc_string() != "" {
                        meta.combine(self.execute_slice(slice, slice_id))
                    } else {
                        meta
                    }
                },
            )
        } else {
            slices
                .into_par_iter()
                .enumerate()
                .fold(
                    || MetaData::new(),
                    |meta: MetaData, (id, slice): (usize, Slice<'ctx>)| {
                        let slice_id = slice_id_offset + id;
                        self.initialize_traces_function_slice_folder(target_name, slice_id)
                            .unwrap();
                        if slice.instr.debug_loc_string() != "" {
                            meta.combine(self.execute_slice(slice, slice_id))
                        } else {
                            meta
                        }
                    },
                )
                .progress_with(pb)
                .reduce(|| MetaData::new(), MetaData::combine)
        }
    }

    pub fn execute_target_slices_map(&self, target_slices_map: HashMap<String, (usize, Vec<Slice<'ctx>>)>) -> MetaData {
        let num_targets = target_slices_map.len();

        // Execute each target function in serial
        if self.options.use_serial() {
            println!("Processing {} Functions...\r", num_targets);
            target_slices_map
                .into_iter()
                .fold(MetaData::new(), |meta, (target_name, (offset, slices))| {
                    meta.combine(self.execute_target_slices(&target_name, offset, slices))
                })
        } else {
            let style = ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:50.green/white} {pos:>5}/{len:5} {percent}% {msg}")
                .progress_chars("##-");
            let pb = ProgressBar::new(num_targets as u64).with_style(style);
            if self.options.use_batch() {
                pb.set_message("Batch functions");
            } else {
                pb.set_message("Total Functions");
            }

            target_slices_map
                .into_par_iter()
                .fold(
                    || MetaData::new(),
                    |meta, (target_name, (offset, slices))| {
                        meta.combine(self.execute_target_slices(&target_name, offset, slices))
                    },
                )
                .progress_with(pb)
                .reduce(|| MetaData::new(), MetaData::combine)
        }
    }
}
