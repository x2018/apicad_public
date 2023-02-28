use llir::values::*;
use serde_json::json;
use std::rc::Rc;
use std::time::SystemTime;

use crate::semantics::rced::*;

#[derive(Clone, Debug)]
pub struct TraceNode<'ctx> {
    pub instr: Instruction<'ctx>,
    pub semantics: Semantics,
    pub result: Option<Rc<Value>>,
}

pub type Trace<'ctx> = Vec<TraceNode<'ctx>>;

pub struct TraceWithTarget<'ctx> {
    pub trace: Trace<'ctx>,
    pub target_index: usize,
}

impl<'ctx> TraceWithTarget<'ctx> {
    pub fn new(trace: Trace<'ctx>, target_index: usize) -> Self {
        Self { trace, target_index }
    }

    pub fn target(&self) -> &TraceNode<'ctx> {
        &self.trace[self.target_index]
    }

    pub fn to_json(&self) -> Result<serde_json::Value, String> {
        let mut timeout = false;
        let start_time = SystemTime::now();
        let mut instrs_info: Vec<serde_json::Value> = Vec::new();
        for node in self.trace.iter() {
            match start_time.elapsed() {
                Ok(elapsed) => {
                    if elapsed.as_secs() as usize >= 3 {
                        timeout = true;
                    }
                }
                Err(_) => {}
            }
            if timeout {
                return Err("Cannot dump json".to_string());
            }
            instrs_info.push(json!({
                "loc": node.instr.debug_loc_string(),
                "sem": node.semantics,
                "res": node.result
            }));
        }
        let json_value = json!({
            "instrs": instrs_info,
            "target": self.target_index,
        });
        Ok(json_value)
    }

    pub fn block_trace(&self) -> Vec<Block<'ctx>> {
        let mut bt = vec![];
        for node in &self.trace {
            let curr_block = node.instr.parent_block();
            if bt.is_empty() || curr_block != bt[bt.len() - 1] {
                bt.push(curr_block);
            }
        }
        bt
    }
}
