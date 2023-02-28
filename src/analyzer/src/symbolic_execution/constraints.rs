use std::collections::HashMap;

use crate::semantics::rced::*;

#[derive(Debug, Clone)]
pub struct Constraint {
    pub cond: Comparison,
    pub branch: bool,
}

pub type Constraints = Vec<Constraint>;

pub trait ConstraintsTrait {
    fn sat(&self, init_symbol_id: usize) -> bool;
}

impl ConstraintsTrait for Constraints {
    fn sat(&self, init_symbol_id: usize) -> bool {
        use z3::*;
        // Note: z3 crate has its own mutex lock
        let z3_ctx = Context::new(&z3::Config::default());
        let solver = Solver::new(&z3_ctx);
        let mut symbol_map = HashMap::new();
        let mut symbol_id = init_symbol_id as u32;
        for Constraint { cond, branch } in self.iter() {
            match cond.into_z3_ast(&mut symbol_map, &mut symbol_id, &z3_ctx) {
                Some(cond) => {
                    let formula = if *branch { cond } else { cond.not() };
                    solver.assert(&formula);
                }
                _ => (),
            }
        }
        match solver.check() {
            // xxx: TODO: leverage the result of solver.get_model()?
            SatResult::Sat | SatResult::Unknown => true,
            _ => false,
        }
    }
}
