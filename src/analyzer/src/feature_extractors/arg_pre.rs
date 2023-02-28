use serde_json::json;
use std::collections::HashSet;

use crate::feature_extraction::*;
use crate::feature_extractors::retval::get_br_cond;
use crate::feature_extractors::retval::num_of_value;
use crate::semantics::boxed::*;

pub struct ArgumentPreconditionFeatureExtractor;

impl ArgumentPreconditionFeatureExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl FeatureExtractor for ArgumentPreconditionFeatureExtractor {
    fn name(&self) -> String {
        format!("arg.pre")
    }

    fn filter<'ctx>(&self, _: bool) -> bool {
        true
    }

    fn extract(&self, _: usize, _: &Slice, trace: &Trace) -> serde_json::Value {
        let mut has_relation = false; // if there are some realtionships between each arguments
        let mut relations = vec![]; // record the relations between each arguments
        let mut checked = vec![]; // false;
        let mut compared_with_const = vec![]; // 0;
        let mut compared_with_non_const = vec![]; // false;
        let mut arg_check_cond = vec![]; // "ne" | "eq" ...
        let mut is_constant = vec![]; // false;
        let mut is_alloca = vec![]; // false;
        let mut is_global = vec![];
        let mut arg_value = vec![];

        // Get the arguments
        let arguments = trace.target_args();

        for _ in arguments.iter() {
            checked.push(false);
            compared_with_const.push(0);
            compared_with_non_const.push(false);
            arg_check_cond.push("".to_string());
            is_constant.push(false);
            is_alloca.push(false);
            is_global.push(false);
            arg_value.push(-1);
        }

        let args_to_check = get_args_to_check(&arguments, 3);

        get_arg_relations(&args_to_check, &mut has_relation, &mut relations);

        for (i, args) in args_to_check.iter().enumerate() {
            for arg in args {
                // Setup kind of arguments
                get_arg_type(
                    &arg,
                    &mut is_constant[i],
                    &mut is_alloca[i],
                    &mut is_global[i],
                    &mut arg_value[i],
                    3,
                );
                if is_constant[i] && is_alloca[i] {
                    break;
                }
            }
        }

        // Checks
        for (instr_i, instr) in trace.iter_instrs_from_target(TraceIterDirection::Backward) {
            match &instr.sem {
                Semantics::ICmp { pred, op0, op1 } => {
                    for (i, args) in args_to_check.iter().enumerate() {
                        for arg in args {
                            // We don't do check if the argument is constant
                            if is_constant[i] {
                                continue;
                            }
                            let arg_is_op0 = &**op0 == arg;
                            let arg_is_op1 = &**op1 == arg;
                            if arg_is_op0 || arg_is_op1 {
                                checked[i] = true;

                                let op0_num = num_of_value(&op0);
                                let op1_num = num_of_value(&op1);
                                if let Some(num) = op0_num.or(op1_num) {
                                    compared_with_const[i] = num;

                                    // Search for a branch instruction after the icmp. Only go 5 steps forward.
                                    for (_, maybe_br) in trace
                                        .iter_instrs_from(TraceIterDirection::Forward, instr_i)
                                        .iter()
                                        .take(5)
                                    {
                                        match &maybe_br.sem {
                                            Semantics::CondBr { cond, br } => {
                                                if &**cond == &instr.res.clone().unwrap() {
                                                    arg_check_cond[i] = get_br_cond(*pred, br);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                } else {
                                    compared_with_non_const[i] = true;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        json!({
            "has_relation": has_relation,
            "relations": relations,
            "arg_num": arguments.len(),
            "feature": arguments.iter().enumerate().map(|(i, _)| json!({
                "check": {
                    "checked": checked[i],
                    "compared_with_const": compared_with_const[i],
                    "compared_with_non_const": compared_with_non_const[i],
                    "check_cond": arg_check_cond[i],
                },
                "is_constant": is_constant[i],
                "is_alloca": is_alloca[i],
                "is_global": is_global[i],
                "arg_value": arg_value[i],
            })).collect::<Vec<_>>(),
        })
    }
}

fn arg_to_check(arg: &Value, depth: usize) -> Vec<Value> {
    if depth == 0 {
        vec![]
    } else {
        match arg {
            _ => vec![arg.clone()],
        }
    }
}

pub fn get_args_to_check(args: &Vec<&Value>, depth: usize) -> Vec<Vec<Value>> {
    if depth == 0 {
        vec![]
    } else {
        let mut args_to_check = vec![];
        for arg in args {
            let arg_to_check = arg_to_check(arg, depth);
            args_to_check.push(arg_to_check);
        }
        args_to_check
    }
}

fn get_arg_type(
    arg: &Value,
    is_constant: &mut bool,
    is_alloca: &mut bool,
    is_global: &mut bool,
    arg_value: &mut i64,
    depth: usize,
) {
    if depth > 0 {
        // Setup kind of argument
        match arg {
            Value::ConstSym(_) | Value::Func(_) | Value::Asm => {
                *is_constant = true;
            }
            Value::Null => {
                *is_constant = true;
                *arg_value = -12345; // Different with Int 0 in sense.
            }
            Value::Int(value) => {
                *is_constant = true;
                *arg_value = *value;
            }
            Value::GEP { loc, .. } => {
                get_arg_type(&*loc, is_constant, is_alloca, is_global, arg_value, depth - 1);
            }
            Value::Alloc(_) => {
                *is_alloca = true;
            }
            Value::Arg(_) | Value::Glob(_) | Value::GlobSym(_) => {
                *is_global = true;
            }
            _ => {}
        }
    }
}

fn get_argval(arg: &Value, depth: usize) -> HashSet<Value> {
    let mut val = HashSet::new();
    if depth > 0 {
        match arg {
            Value::GEP { loc, .. } => {
                for v in get_argval(&*loc, depth - 1).iter() {
                    val.insert(v.clone());
                }
            }
            Value::Bin { op0, op1, .. } | Value::ICmp { op0, op1, .. } => {
                for v in get_argval(&*op0, depth - 1).iter() {
                    val.insert(v.clone());
                }
                for v in get_argval(&*op1, depth - 1).iter() {
                    val.insert(v.clone());
                }
            }
            Value::Call { args, .. } => {
                for arg in args {
                    for v in get_argval(&arg, depth - 1).iter() {
                        val.insert(v.clone());
                    }
                }
            }
            Value::Unknown => {}
            _ => {
                val.insert(arg.clone());
            }
        }
    }
    val
}

fn get_argvals(args: &Vec<Value>) -> HashSet<Value> {
    let mut argvals = HashSet::new();
    for arg in args {
        for val in get_argval(&arg, 3).iter() {
            argvals.insert(val.clone());
        }
    }
    argvals
}

fn get_arg_relations(args_to_check: &Vec<Vec<Value>>, has_relation: &mut bool, relations: &mut Vec<Vec<usize>>) {
    let arg_num = args_to_check.len();
    let mut argvals: Vec<HashSet<Value>> = Vec::new();
    if arg_num > 1 {
        argvals.push(get_argvals(&args_to_check[0]));
        let mut i = 0;
        while i < (arg_num - 1) {
            let mut j = i + 1;
            while j < arg_num {
                if j <= argvals.len() {
                    argvals.push(get_argvals(&args_to_check[j]));
                }
                if !argvals[i].is_disjoint(&argvals[j]) {
                    *has_relation = true;
                    relations.push(vec![i, j]);
                }
                j += 1;
            }
            i += 1;
        }
    }
}
