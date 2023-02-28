use serde_json::json;
use std::collections::HashSet;

use crate::feature_extraction::*;
use crate::semantics::boxed::*;
use crate::semantics::*;

pub struct ReturnValueFeatureExtractor;

impl ReturnValueFeatureExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl FeatureExtractor for ReturnValueFeatureExtractor {
    fn name(&self) -> String {
        "retval".to_string()
    }

    // Check whether the function has a valid type of return value
    fn filter<'ctx>(&self, has_return_type: bool) -> bool {
        has_return_type
    }

    fn extract(&self, _: usize, _: &Slice, trace: &Trace) -> serde_json::Value {
        // For the check of return value
        let mut checked = false;
        let mut indir_checked = false;
        let mut br_cond = "".to_string();
        let mut compared_with_const = 0;
        let mut compared_with_non_const = false;

        // For the context of return value
        let mut used_in_call = false;
        let mut used_in_bin = false;
        let mut stored_not_local = false;
        let mut derefed_read = false;
        let mut derefed_write = false;
        let mut returned = false;
        let mut indir_returned = false;

        // States
        let mut child_ptrs: HashSet<Value> = HashSet::new();
        let mut tracked_values: HashSet<Value> = HashSet::new();
        let mut icmp = None;
        let mut had_used = 0;

        // Maybe `None` value?
        let retval: Value;
        if let Some(value) = trace.target_result().clone() {
            retval = value;
        } else {
            return json!({});
        }

        // Start iterating from the target node forward
        for (_, instr) in trace.iter_instrs_from_target(TraceIterDirection::Forward) {
            match &instr.sem {
                Semantics::ICmp { op0, op1, .. } => {
                    if had_used <= 1 && !derefed_write && !derefed_read {
                        let retval_is_op0 = **op0 == retval;
                        let retval_is_op1 = **op1 == retval;
                        if checked == false && (retval_is_op0 || retval_is_op1) {
                            checked = true;
                            icmp = Some(instr.res.clone().unwrap());
                        } else if tracked_values.contains(&**op0)
                            || tracked_values.contains(&**op1)
                            || child_ptrs.contains(&**op0)
                            || child_ptrs.contains(&**op1)
                        {
                            indir_checked = true;
                        }
                    }
                }
                Semantics::CondBr { cond, br } => {
                    if let Some(icmp) = &icmp {
                        if &**cond == icmp {
                            if let Some((pred, op0, op1)) = icmp_pred_op0_op1(icmp) {
                                let op0_num = num_of_value(&op0);
                                let op1_num = num_of_value(&op1);
                                if let Some(num) = op0_num.or(op1_num) {
                                    compared_with_const = num;
                                } else {
                                    compared_with_non_const = true;
                                }
                                br_cond = get_br_cond(pred, br);
                            }
                        }
                    }
                }
                Semantics::Call { args, .. } => {
                    if args
                        .iter()
                        .find(|a| &***a == &retval || child_ptrs.contains(&**a))
                        .is_some()
                    {
                        used_in_call = true;
                        had_used += 1;
                    }
                }
                Semantics::Load { loc } => {
                    if **loc == retval || child_ptrs.contains(&**loc) {
                        derefed_read = true;
                    }
                }
                Semantics::Store { loc, val } => {
                    if **loc == retval || child_ptrs.contains(&**loc) {
                        derefed_write = true;
                    } else if **val == retval || child_ptrs.contains(&**val) {
                        let loc = *loc.clone();
                        match &loc {
                            Value::Sym(_) | Value::Alloc(_) => {
                                tracked_values.insert(loc);
                            }
                            Value::Arg(_) | Value::Glob(_) | Value::GlobSym(_) => {
                                tracked_values.insert(loc);
                                stored_not_local = true;
                            }
                            Value::GEP { loc, .. } => {
                                tracked_values.insert(*loc.clone());
                                stored_not_local = tracked_not_local(loc);
                            }
                            _ => {}
                        }
                    }
                }
                Semantics::GEP { loc, .. } => {
                    if **loc == retval || child_ptrs.contains(&**loc) {
                        // GEP only performs address calculation and does not access memory
                        child_ptrs.insert(instr.res.clone().unwrap());
                    }
                }
                Semantics::Ret { op } => {
                    if let Some(op) = op {
                        if retval == **op {
                            returned = true;
                        } else if tracked_values.contains(&**op) || child_ptrs.contains(&**op) {
                            indir_returned = true;
                        }
                    }
                }
                Semantics::Bin { op0, op1, .. } => {
                    let arg_is_op0 = &**op0 == &retval;
                    let arg_is_op1 = &**op1 == &retval;
                    if arg_is_op0 || arg_is_op1 {
                        used_in_bin = true;
                        // track the binary instr
                        child_ptrs.insert(instr.res.clone().unwrap());
                    }
                }
                _ => {}
            }
        }

        json!({
            "check": {
                "checked": checked,
                "indir_checked": indir_checked,
                "check_cond": br_cond,
                "compared_with_const": compared_with_const,
                "compared_with_non_const": compared_with_non_const,
            },
            "ctx": {
                "used_in_call": used_in_call,
                "used_in_bin": used_in_bin,
                // "stored": stored,
                "stored_not_local": stored_not_local,
                "derefed_read": derefed_read,
                "derefed_write": derefed_write,
                "returned": returned,
                "indir_returned": indir_returned,
            },
        })
    }
}

pub fn num_of_value(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i) => Some(i.clone()),
        Value::Null => Some(0),
        _ => None,
    }
}

pub fn get_br_cond(pred: Predicate, br: &Branch) -> String {
    let br_cond: &str;
    if *br == Branch::Then {
        br_cond = match pred {
            Predicate::EQ => "eq",
            Predicate::NE => "ne",
            Predicate::SGE | Predicate::UGE => "ge",
            Predicate::SGT | Predicate::UGT => "gt",
            Predicate::SLE | Predicate::ULE => "le",
            Predicate::SLT | Predicate::ULT => "lt",
        };
    } else {
        br_cond = match pred {
            Predicate::EQ => "ne",
            Predicate::NE => "eq",
            Predicate::SGE | Predicate::UGE => "lt",
            Predicate::SGT | Predicate::UGT => "le",
            Predicate::SLE | Predicate::ULE => "gt",
            Predicate::SLT | Predicate::ULT => "ge",
        };
    }
    br_cond.to_string()
}

fn icmp_pred_op0_op1(v: &Value) -> Option<(Predicate, Value, Value)> {
    match v {
        Value::ICmp { pred, op0, op1 } => Some((pred.clone(), *op0.clone(), *op1.clone())),
        _ => None,
    }
}

fn tracked_not_local(v: &Value) -> bool {
    match v {
        Value::Arg(_) | Value::Glob(_) | Value::GlobSym(_) => {
            return true;
        }
        Value::GEP { loc, .. } => {
            return tracked_not_local(loc);
        }
        _ => {}
    }
    false
}
