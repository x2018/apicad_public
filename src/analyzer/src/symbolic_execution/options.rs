use crate::options::*;

pub trait SymbolicExecutionOptions: GeneralOptions + IOOptions + Send + Sync {
    fn slice_depth(&self) -> usize;

    fn max_timeout(&self) -> usize;

    fn max_node_per_trace(&self) -> usize;

    fn max_explored_trace_per_slice(&self) -> usize;

    fn max_trace_per_slice(&self) -> usize;

    fn step_in_anytime(&self) -> bool;

    fn is_rough(&self) -> bool;

    fn not_random_scheduling(&self) -> bool;
}
