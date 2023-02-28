use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;

pub type UnaOp = llir::values::UnaryOpcode;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(remote = "llir::values::UnaryOpcode")]
pub enum UnaryOpcodeDef {
    FNeg,
    Trunc,
    ZExt,
    SExt,
    FPToUI,
    FPToSI,
    UIToFP,
    SIToFP,
    FPTrunc,
    FPExt,
    PtrToInt,
    IntToPtr,
    BitCast,
}

pub type BinOp = llir::values::BinaryOpcode;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(remote = "llir::values::BinaryOpcode")]
enum BinaryOpcodeDef {
    // Arithmatics
    Add,
    Sub,
    Mul,
    UDiv,
    SDiv,
    URem,
    SRem,
    // Floating point
    FAdd,
    FSub,
    FMul,
    FDiv,
    FRem,
    // Bitwise operation
    Shl,
    LShr,
    AShr,
    And,
    Or,
    Xor,
}

pub type Predicate = llir::values::ICmpPredicate;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(remote = "llir::values::ICmpPredicate")]
pub enum PredicateDef {
    EQ,
    NE,
    SGE,
    UGE,
    SGT,
    UGT,
    SLE,
    ULE,
    SLT,
    ULT,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Branch {
    Then,
    Else,
}

impl Branch {
    pub fn is_then(&self) -> bool {
        match self {
            Self::Then => true,
            _ => false,
        }
    }

    pub fn is_else(&self) -> bool {
        match self {
            Self::Else => true,
            _ => false,
        }
    }
}

macro_rules! decl_value_with_wrapper {
    ($wrapper:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Value {
            Arg(usize),      // Argument ID
            ConstSym(usize), // Constant Symbol ID
            Sym(usize),      // Temporary Symbol ID
            GlobSym(usize),  // Global Symbol ID
            Glob(String),    // Global Value Name
            Func(String),    // Function Name
            Alloc(usize),    // Local alloca ID
            FuncPtr,
            Asm,
            Int(i64),
            Null,
            GEP {
                loc: $wrapper<Value>,
                indices: Vec<$wrapper<Value>>,
            },
            Bin {
                #[serde(with = "BinaryOpcodeDef")]
                op: BinOp,
                op0: $wrapper<Value>,
                op1: $wrapper<Value>,
            },
            ICmp {
                #[serde(with = "PredicateDef")]
                pred: Predicate,
                op0: $wrapper<Value>,
                op1: $wrapper<Value>,
            },
            Call {
                id: usize,
                func: $wrapper<Value>,
                args: Vec<$wrapper<Value>>,
            },
            Unknown,
        }

        impl Value {
            pub fn as_comparison(&self) -> Option<Comparison> {
                match self {
                    Value::ICmp { pred, op0, op1 } => Some(Comparison {
                        pred: *pred,
                        op0: op0.clone(),
                        op1: op1.clone(),
                    }),
                    _ => None,
                }
            }

            pub fn contains(&self, value: &Value) -> bool {
                match value {
                    Value::GEP { loc, .. } => {
                        if &**loc == self {
                            true
                        } else {
                            self.contains(loc)
                        }
                    }
                    _ => self == value,
                }
            }

            pub fn eval_constant_value<'ctx>(
                &self,
            ) -> Option<i64> {
                match self {
                    Value::Int(i) => Some(*i),
                    Value::Null => Some(0),
                    Value::Bin {op, op0, op1} => {
                        match (
                            op0.eval_constant_value(),
                            op1.eval_constant_value()
                        ) {
                            (Some(op0), Some(op1)) => match op {
                                BinOp::Add => Some(op0 + op1),
                                BinOp::Sub => Some(op0 - op1),
                                BinOp::Mul => Some(op0 * op1),
                                BinOp::UDiv | BinOp::SDiv => Some(op0 / op1),
                                BinOp::URem | BinOp::SRem => Some(op0 % op1),
                                _ => None,
                            },
                            _ => None,
                        }
                    },
                    _ => None,
                }
            }

            pub fn into_z3_ast<'ctx>(
                &self,
                symbol_map: &mut HashMap<Value, z3::Symbol>,
                symbol_id: &mut u32,
                z3_ctx: &'ctx z3::Context,
            ) -> Option<z3::ast::Int<'ctx>> {
                use z3::*;
                match self {
                    Value::Int(i) => Some(ast::Int::from_i64(z3_ctx, *i)),
                    Value::Null => Some(ast::Int::from_i64(z3_ctx, 0)),
                    Value::Bin { op, op0, op1 } => {
                        match (
                            op0.into_z3_ast(symbol_map, symbol_id, z3_ctx),
                            op1.into_z3_ast(symbol_map, symbol_id, z3_ctx),
                        ) {
                            (Some(op0), Some(op1)) => match op {
                                BinOp::Add => Some(ast::Int::add(z3_ctx, &[&op0, &op1])),
                                BinOp::Sub => Some(ast::Int::sub(z3_ctx, &[&op0, &op1])),
                                BinOp::Mul => Some(ast::Int::mul(z3_ctx, &[&op0, &op1])),
                                BinOp::UDiv | BinOp::SDiv => Some(op0.div(&op1)),
                                BinOp::URem | BinOp::SRem => Some(op0.rem(&op1)),
                                _ => None,
                            },
                            _ => None,
                        }
                    }
                    Value::Unknown => None,
                    Value::Sym(id) | Value::ConstSym(id) | Value::GlobSym(id) => {
                        let symbol = symbol_map.entry(self.clone()).or_insert_with(|| {
                            let result = *id as u32;
                            Symbol::Int(result)
                        });
                        Some(ast::Int::new_const(z3_ctx, symbol.clone()))
                    }
                    _ => {
                        let symbol = symbol_map.entry(self.clone()).or_insert_with(|| {
                            let result = *symbol_id;
                            *symbol_id += 1;
                            Symbol::Int(result)
                        });
                        Some(ast::Int::new_const(z3_ctx, symbol.clone()))
                    }
                }
            }
        }

        #[derive(Debug, Clone)]
        pub struct Comparison {
            pred: Predicate,
            op0: $wrapper<Value>,
            op1: $wrapper<Value>,
        }

        impl Comparison {
            pub fn into_z3_ast<'ctx>(
                &self,
                symbol_map: &mut HashMap<Value, z3::Symbol>,
                symbol_id: &mut u32,
                z3_ctx: &'ctx z3::Context,
            ) -> Option<z3::ast::Bool<'ctx>> {
                use z3::ast::Ast;
                let Comparison { pred, op0, op1 } = self;
                let z3_op0 = op0.into_z3_ast(symbol_map, symbol_id, z3_ctx);
                let z3_op1 = op1.into_z3_ast(symbol_map, symbol_id, z3_ctx);
                match (z3_op0, z3_op1) {
                    (Some(op0), Some(op1)) => match pred {
                        Predicate::EQ => Some(op0._eq(&op1)),
                        Predicate::NE => Some(op0._eq(&op1).not()),
                        Predicate::SGE | Predicate::UGE => Some(op0.ge(&op1)),
                        Predicate::SGT | Predicate::UGT => Some(op0.gt(&op1)),
                        Predicate::SLE | Predicate::ULE => Some(op0.le(&op1)),
                        Predicate::SLT | Predicate::ULT => Some(op0.lt(&op1)),
                    },
                    _ => None,
                }
            }

            pub fn evaluate_cond<'ctx>(
                &self,
                then: bool,
            ) -> bool {
                let Comparison { pred, op0, op1 } = self;
                let op0 = op0.eval_constant_value();
                let op1 = op1.eval_constant_value();
                match (op0, op1) {
                    (Some(i), Some(j)) => match pred {
                        Predicate::EQ => {
                            if i == j {
                                if then {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                if then {
                                    false
                                } else {
                                    true
                                }
                            }
                        },
                        Predicate::NE => {
                            if i != j {
                                if then {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                if then {
                                    false
                                } else {
                                    true
                                }
                            }
                        },
                        Predicate::SGE | Predicate::UGE => {
                            if i >= j {
                                if then {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                if then {
                                    false
                                } else {
                                    true
                                }
                            }
                        },
                        Predicate::SGT | Predicate::UGT => {
                            if i > j {
                                if then {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                if then {
                                    false
                                } else {
                                    true
                                }
                            }
                        },
                        Predicate::SLE | Predicate::ULE => {
                            if i <= j {
                                if then {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                if then {
                                    false
                                } else {
                                    true
                                }
                            }
                        },
                        Predicate::SLT | Predicate::ULT => {
                            if i < j {
                                if then {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                if then {
                                    false
                                } else {
                                    true
                                }
                            }
                        },
                    },
                    _ => true,
                }
            }
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Semantics {
            Call {
                func: $wrapper<Value>,
                args: Vec<$wrapper<Value>>,
            },
            ICmp {
                #[serde(with = "PredicateDef")]
                pred: Predicate,
                op0: $wrapper<Value>,
                op1: $wrapper<Value>,
            },
            CondBr {
                cond: $wrapper<Value>,
                br: Branch,
            },
            Switch {
                cond: $wrapper<Value>,
            },
            Ret {
                op: Option<$wrapper<Value>>,
            },
            Store {
                loc: $wrapper<Value>,
                val: $wrapper<Value>,
            },
            Load {
                loc: $wrapper<Value>,
            },
            GEP {
                loc: $wrapper<Value>,
                indices: Vec<$wrapper<Value>>,
            },
            Una {
                #[serde(with = "UnaryOpcodeDef")]
                op: UnaOp,
                op0: $wrapper<Value>,
            },
            Bin {
                #[serde(with = "BinaryOpcodeDef")]
                op: BinOp,
                op0: $wrapper<Value>,
                op1: $wrapper<Value>,
            },
        }

        impl Semantics {
            pub fn call_args(&self) -> Vec<&Value> {
                match self {
                    Semantics::Call { args, .. } => args.iter().map(|v| &**v).collect(),
                    _ => panic!("Target is not a call"),
                }
            }
        }
    };
}

pub mod boxed {
    //! The Value & Semantics with wrapper using Box

    use super::*;

    decl_value_with_wrapper!(Box);
}

pub mod rced {
    //! The Value & Semantics with wrapper using Rc

    use super::*;

    decl_value_with_wrapper!(Rc);
}
