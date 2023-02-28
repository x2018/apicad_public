use serde_json::json;
use std::collections::HashSet;

use crate::feature_extraction::*;
use crate::feature_extractors::arg_pre::get_args_to_check;
use crate::semantics::boxed::*;

pub struct ArgumentPostconditionFeatureExtractor;

impl ArgumentPostconditionFeatureExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl FeatureExtractor for ArgumentPostconditionFeatureExtractor {
    fn name(&self) -> String {
        format!("arg.post")
    }

    fn filter<'ctx>(&self, _: bool) -> bool {
        true
    }

    fn extract(&self, _: usize, _: &Slice, trace: &Trace) -> serde_json::Value {
        let mut used_in_check = vec![]; // false;
        let mut derefed_read = vec![]; // false;
        let mut derefed_write = vec![];
        let mut returned = vec![]; // false;
        let mut indir_returned = vec![]; // false;

        // Helper structures
        let mut child_ptrs: Vec<HashSet<Value>> = Vec::new();
        let mut tracked_values: Vec<HashSet<Value>> = Vec::new();
        let mut had_used = vec![];

        // Get the arguments
        let arguments = trace.target_args();

        for _ in arguments.iter() {
            used_in_check.push(false);
            derefed_read.push(false);
            derefed_write.push(false);
            returned.push(false);
            indir_returned.push(false);
            child_ptrs.push(HashSet::new());
            tracked_values.push(HashSet::new());
            had_used.push(0);
        }

        // Setup kind of arguments
        let args_to_check = get_args_to_check(&arguments, 3);

        // Iterate forward
        for (_, instr) in trace.iter_instrs_from_target(TraceIterDirection::Forward) {
            match &instr.sem {
                Semantics::ICmp { op0, op1, .. } => {
                    for (i, args) in args_to_check.iter().enumerate() {
                        if had_used[i] <= 1 && !derefed_write[i] && !derefed_read[i] {
                            for arg in args {
                                let arg_is_op0 = &**op0 == arg;
                                let arg_is_op1 = &**op1 == arg;
                                if used_in_check[i] == false
                                    && (arg_is_op0
                                        || arg_is_op1
                                        || tracked_values[i].contains(&**op0)
                                        || tracked_values[i].contains(&**op1)
                                        || child_ptrs[i].contains(&**op0)
                                        || child_ptrs[i].contains(&**op1))
                                {
                                    used_in_check[i] = true;
                                }
                            }
                        }
                    }
                }
                Semantics::Ret { op } => {
                    for (i, args) in args_to_check.iter().enumerate() {
                        for arg in args {
                            if let Some(op) = op {
                                if arg == &**op {
                                    returned[i] = true;
                                } else if op.contains(&arg)
                                    || tracked_values[i].contains(&**op)
                                    || child_ptrs[i].contains(&**op)
                                {
                                    indir_returned[i] = true;
                                }
                            }
                        }
                    }
                }
                Semantics::Store { loc, val } => {
                    for (i, args) in args_to_check.iter().enumerate() {
                        for arg in args {
                            if &**loc == arg {
                                derefed_write[i] = true;
                            } else if &**val == arg || child_ptrs[i].contains(&**val) {
                                let loc = *loc.clone();
                                match &loc {
                                    Value::Arg(_)
                                    | Value::Sym(_)
                                    | Value::Glob(_)
                                    | Value::Alloc(_)
                                    | Value::GlobSym(_) => {
                                        tracked_values[i].insert(loc);
                                    }
                                    Value::GEP { loc, .. } => {
                                        tracked_values[i].insert(*loc.clone());
                                    }
                                    _ => {}
                                }
                            } else if child_ptrs[i].contains(&**loc) {
                                derefed_write[i] = true;
                            }
                        }
                    }
                }
                Semantics::Load { loc } => {
                    for (i, args) in args_to_check.iter().enumerate() {
                        for arg in args {
                            if &**loc == arg || child_ptrs[i].contains(&**loc) {
                                derefed_read[i] = true;
                            }
                        }
                    }
                }
                Semantics::GEP { loc, .. } => {
                    for (i, args) in args_to_check.iter().enumerate() {
                        for arg in args {
                            if &**loc == arg {
                                // GEP only performs address calculation and does not access memory
                                child_ptrs[i].insert(instr.res.clone().unwrap());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        json!({
            "feature": arguments.iter().enumerate().map(|(i, _)| json!({
                "used_in_check": used_in_check[i],
                "derefed_read": derefed_read[i],
                "derefed_write": derefed_write[i],
                "returned": returned[i],
                "indir_returned": indir_returned[i],
            })).collect::<Vec<_>>(),
            "arg_num": arguments.len(),
        })
    }
}
