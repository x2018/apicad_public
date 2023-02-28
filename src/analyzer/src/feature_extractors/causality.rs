use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::feature_extraction::*;
use crate::semantics::boxed::*;

#[derive(Clone, Serialize)]
struct CausalityFeatures {
    // Note:
    // 1. in pre.call, this means:
    // Used by target function as argument
    // e.g.
    // 		a = func1(...);
    // 		b = target(..., a, ...);
    // 2. in post.call this means:
    // Use target function as argument
    // e.g.
    // 		c = target(...);
    // 		d = func2(..., c, ...);
    pub used_as_arg: bool,
    // pub use_target_as_arg: bool,

    // Share argument value
    pub share_argument: bool,
}

impl Default for CausalityFeatures {
    fn default() -> Self {
        Self {
            used_as_arg: false,
            share_argument: false,
        }
    }
}

pub struct CausalityFeatureExtractor;

impl CausalityFeatureExtractor {
    pub fn new() -> Self {
        Self
    }
}

impl FeatureExtractor for CausalityFeatureExtractor {
    fn name(&self) -> String {
        format!("causality")
    }

    fn filter<'ctx>(&self, _: bool) -> bool {
        true
    }

    fn extract(&self, _: usize, slice: &Slice, trace: &Trace) -> serde_json::Value {
        let func_instrs = slice.functions.iter().map(|(_, instr)| instr).collect::<HashSet<_>>();
        let pre_related_funcs = find_related_functions(&func_instrs, trace, TraceIterDirection::Backward);
        let post_related_funcs = find_related_functions(&func_instrs, trace, TraceIterDirection::Forward);
        let mut pre_map = serde_json::Map::new();
        let mut post_map = serde_json::Map::new();

        for (func, causality_features) in pre_related_funcs {
            pre_map.insert(
                func.clone(),
                serde_json::to_value(causality_features).expect("Cannot turn causality_pre features into json"),
            );
        }
        for (func, causality_features) in post_related_funcs {
            post_map.insert(
                func.clone(),
                serde_json::to_value(causality_features).expect("Cannot turn causality_post features into json"),
            );
        }

        json!({
            "pre.call": serde_json::Value::Object(pre_map),
            "post.call": serde_json::Value::Object(post_map),
        })
    }
}

fn find_related_functions(
    func_instrs: &HashSet<&String>,
    trace: &Trace,
    direction: TraceIterDirection,
) -> HashMap<String, CausalityFeatures> {
    let mut result = HashMap::new();
    let target_instr = &trace.instrs[trace.target];

    for (_, instr) in trace.iter_instrs_from_target(direction) {
        match &instr.sem {
            Semantics::Call { func, .. } => match &**func {
                Value::Func(f) => {
                    if f.contains("__asan")
                        || f.contains("__sanitizer")
                        || f.contains("__kasan")
                        || f.contains("print")
                        || result.contains_key(f)
                    {
                        continue;
                    }
                    let mut features = CausalityFeatures::default();

                    // Check if sharing argument value.
                    features.share_argument = check_if_share_arguments(instr, target_instr);
                    if direction.is_forward() {
                        // Check if the return value of target function is used by this function.
                        features.used_as_arg = check_a_is_arg_of_b(target_instr, instr);
                    } else {
                        // Check if this function is used as a argument by target function.
                        features.used_as_arg = check_a_is_arg_of_b(instr, target_instr);
                    }

                    if features.share_argument || features.used_as_arg || func_instrs.contains(&instr.loc) {
                        result.insert(f.clone(), features.clone());
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    result
}

// Check if sharing argument value.
pub fn check_if_share_arguments(instr: &Instr, target_instr: &Instr) -> bool {
    let args_1 = tracked_args(instr);
    let args_2 = tracked_args(target_instr);
    if args_1
        .iter()
        .find(|a| args_2.iter().find(|b| a == b).is_some())
        .is_some()
    {
        return true;
    }
    false
}

// Check if the return value of a is used as argument by b.
pub fn check_a_is_arg_of_b(instr: &Instr, target_instr: &Instr) -> bool {
    let retval = (tracked_res(instr), tracked_args(target_instr));
    if let (Some(retvals), args) = retval {
        for retval in retvals {
            if args.iter().find(|a| &***a == retval).is_some() {
                return true;
            }
        }
    }
    false
}

fn tracked_res(instr: &Instr) -> Option<Vec<&Value>> {
    match &instr.res {
        Some(r) => Some(match r {
            Value::Unknown | Value::Null => vec![],
            _ => vec![&r],
        }),
        None => None,
    }
}

fn tracked_args(instr: &Instr) -> Vec<&Value> {
    instr
        .sem
        .call_args()
        .into_iter()
        .map(|a| match a {
            Value::Unknown | Value::Null => vec![],
            _ => vec![a],
        })
        .flatten()
        .collect::<Vec<_>>()
}
