//! The code of a script as Pawn-like text, for reading. It is made by following the stack machine of the code: the values that are pushed
//! become the parameters of the calls, the jumps become `if` and `while` when their shape is simple, and the rest stays as `goto`.
//!
//! The text is a help to understand the assembly, not a program that can be built again: the names of the variables are made up (`arg1`, `local1`,
//! `data3`) and the shape of the structures is a guess.

use crate::amx::Amx;
use crate::assembly::{decode, opcode, target_of, Instruction};
use crate::opcodes::Form;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// A value.
#[derive(Clone, Debug, PartialEq)]
enum Expr {
    Number(i32),
    Arg(i32),
    Local(i32),
    Data(i32),
    Result(u32),
    Binary(Box<Expr>, &'static str, Box<Expr>),
    Unary(&'static str, Box<Expr>),
    Call(String, Vec<Expr>),
    /// The address of a variable, as a parameter that lets a function change it or read an array.
    Address(Box<Expr>),
    Deref(Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Unknown,
}

impl Expr {
    fn text(&self) -> String {
        match self {
            Expr::Number(n) => {
                if *n > 9999 || *n < -9999 {
                    format!("0x{:X}", *n as u32)
                } else {
                    n.to_string()
                }
            }
            Expr::Arg(i) => format!("arg{i}"),
            Expr::Local(i) => format!("local{i}"),
            Expr::Data(i) => format!("data{i}"),
            Expr::Result(i) => format!("result{i}"),
            Expr::Binary(a, op, b) => format!("{} {op} {}", a.wrapped(), b.wrapped()),
            Expr::Unary(op, a) => format!("{op}{}", a.wrapped()),
            Expr::Call(name, args) => format!("{name}({})", args.iter().map(Expr::text).collect::<Vec<_>>().join(", ")),
            Expr::Address(a) => format!("&{}", a.text()),
            Expr::Deref(a) => format!("*{}", a.wrapped()),
            Expr::Index(a, i) => format!("{}[{}]", a.wrapped(), i.text()),
            Expr::Unknown => "?".to_string(),
        }
    }

    fn wrapped(&self) -> String {
        match self {
            Expr::Binary(..) => format!("({})", self.text()),
            _ => self.text(),
        }
    }

    /// The opposite condition.
    fn negated(&self) -> Expr {
        match self {
            Expr::Binary(a, op, b) => {
                let flipped = match *op {
                    "==" => "!=",
                    "!=" => "==",
                    "<" => ">=",
                    ">=" => "<",
                    ">" => "<=",
                    "<=" => ">",
                    _ => return Expr::Unary("!", Box::new(self.clone())),
                };
                Expr::Binary(a.clone(), flipped, b.clone())
            }
            Expr::Unary("!", inner) => (**inner).clone(),
            _ => Expr::Unary("!", Box::new(self.clone())),
        }
    }

    fn uses_result(&self, id: u32) -> usize {
        match self {
            Expr::Result(r) => usize::from(*r == id),
            Expr::Binary(a, _, b) | Expr::Index(a, b) => a.uses_result(id) + b.uses_result(id),
            Expr::Unary(_, a) | Expr::Address(a) | Expr::Deref(a) => a.uses_result(id),
            Expr::Call(_, args) => args.iter().map(|a| a.uses_result(id)).sum(),
            _ => 0,
        }
    }

    fn replace_result(&self, id: u32, with: &Expr) -> Expr {
        let go = |e: &Expr| Box::new(e.replace_result(id, with));
        match self {
            Expr::Result(r) if *r == id => with.clone(),
            Expr::Binary(a, op, b) => Expr::Binary(go(a), op, go(b)),
            Expr::Index(a, b) => Expr::Index(go(a), go(b)),
            Expr::Unary(op, a) => Expr::Unary(op, go(a)),
            Expr::Address(a) => Expr::Address(go(a)),
            Expr::Deref(a) => Expr::Deref(go(a)),
            Expr::Call(name, args) => Expr::Call(name.clone(), args.iter().map(|a| a.replace_result(id, with)).collect()),
            other => other.clone(),
        }
    }
}

#[derive(Clone, Debug)]
enum Stmt {
    Assign(Expr, Expr),
    /// A call whose value is kept in a result.
    Let(u32, Expr),
    Run(Expr),
    Return(Option<Expr>),
    Label(i64),
    Goto(i64),
    /// Jumps to a label when the condition is true.
    Branch(Expr, i64),
    Switch(Expr, Vec<(i32, i64)>, i64),
    Note(String),
}

struct Machine<'a> {
    pri: Expr,
    alt: Expr,
    stack: Vec<Expr>,
    out: Vec<Stmt>,
    results: u32,
    natives: &'a dyn Fn(i32) -> String,
    functions: &'a BTreeMap<i64, String>,
}

fn variable(offset: i32) -> Expr {
    if offset >= 12 {
        Expr::Arg((offset - 12) / 4 + 1)
    } else {
        Expr::Local((-offset) / 4)
    }
}

impl Machine<'_> {
    fn pop(&mut self) -> Expr {
        self.stack.pop().unwrap_or(Expr::Unknown)
    }

    fn call(&mut self, name: String, count: usize) {
        let args: Vec<Expr> = (0..count).map(|_| self.pop()).collect();
        self.results += 1;
        let id = self.results;
        self.out.push(Stmt::Let(id, Expr::Call(name, args)));
        self.pri = Expr::Result(id);
    }

    fn binary(&mut self, op: &'static str) {
        self.pri = Expr::Binary(Box::new(self.pri.clone()), op, Box::new(self.alt.clone()));
    }

    fn compare(&mut self, op: &'static str, constant: Option<i32>) {
        let right = constant.map_or_else(|| self.alt.clone(), Expr::Number);
        self.pri = Expr::Binary(Box::new(self.pri.clone()), op, Box::new(right));
    }

    fn branch(&mut self, at: &Instruction, cell: i32, jump_when: Expr) {
        self.out.push(Stmt::Branch(jump_when, target_of(at.at + 4, cell)));
    }

    /// Runs one instruction.
    fn step(&mut self, at: &Instruction) {
        let Some((name, form)) = opcode(at.number) else { return };
        let c = &at.cells;
        // A value packed in the opcode has 16 bits and a sign: a place of the stack is often negative.
        let first = c.first().copied().map_or(0, |v| if form == Form::Packed { i32::from(v as u16 as i16) } else { v });
        match name {
            "proc" | "break" | "nop" | "align.pri" | "align.alt" => {}
            "const" => self.out.push(Stmt::Assign(Expr::Data(first / 4), Expr::Number(c[1]))),
            "const.s" => self.out.push(Stmt::Assign(variable(first), Expr::Number(c[1]))),
            "const.pri" => self.pri = Expr::Number(first),
            "const.alt" => self.alt = Expr::Number(first),
            "const.p.pri" => self.pri = Expr::Number(first),
            "const.p.alt" => self.alt = Expr::Number(first),
            "load.pri" | "load.p.pri" => self.pri = Expr::Data(first / 4),
            "load.alt" | "load.p.alt" => self.alt = Expr::Data(first / 4),
            "load.s.pri" | "load.p.s.pri" => self.pri = variable(first),
            "load.s.alt" | "load.p.s.alt" => self.alt = variable(first),
            "load.both" => {
                self.pri = Expr::Data(first / 4);
                self.alt = Expr::Data(c[1] / 4);
            }
            "load.s.both" => {
                self.pri = variable(first);
                self.alt = variable(c[1]);
            }
            "lref.s.pri" | "lref.p.s.pri" => self.pri = Expr::Deref(Box::new(variable(first))),
            "lref.s.alt" | "lref.p.s.alt" => self.alt = Expr::Deref(Box::new(variable(first))),
            "lref.pri" | "lref.p.pri" => self.pri = Expr::Deref(Box::new(Expr::Data(first / 4))),
            "lref.alt" | "lref.p.alt" => self.alt = Expr::Deref(Box::new(Expr::Data(first / 4))),
            "load.i" => self.pri = Expr::Deref(Box::new(self.pri.clone())),
            "addr.pri" | "addr.p.pri" => self.pri = Expr::Address(Box::new(variable(first))),
            "addr.alt" | "addr.p.alt" => self.alt = Expr::Address(Box::new(variable(first))),
            "stor.pri" | "stor.p.pri" => self.out.push(Stmt::Assign(Expr::Data(first / 4), self.pri.clone())),
            "stor.alt" | "stor.p.alt" => self.out.push(Stmt::Assign(Expr::Data(first / 4), self.alt.clone())),
            "stor.s.pri" | "stor.p.s.pri" => self.out.push(Stmt::Assign(variable(first), self.pri.clone())),
            "stor.s.alt" | "stor.p.s.alt" => self.out.push(Stmt::Assign(variable(first), self.alt.clone())),
            "sref.s.pri" | "sref.p.s.pri" => self.out.push(Stmt::Assign(Expr::Deref(Box::new(variable(first))), self.pri.clone())),
            "sref.s.alt" | "sref.p.s.alt" => self.out.push(Stmt::Assign(Expr::Deref(Box::new(variable(first))), self.alt.clone())),
            "sref.pri" | "sref.p.pri" => self.out.push(Stmt::Assign(Expr::Deref(Box::new(Expr::Data(first / 4))), self.pri.clone())),
            "sref.alt" | "sref.p.alt" => self.out.push(Stmt::Assign(Expr::Deref(Box::new(Expr::Data(first / 4))), self.alt.clone())),
            "stor.i" => self.out.push(Stmt::Assign(Expr::Deref(Box::new(self.alt.clone())), self.pri.clone())),
            "lidx" => self.pri = Expr::Index(Box::new(self.alt.clone()), Box::new(self.pri.clone())),
            "idxaddr" => self.pri = Expr::Address(Box::new(Expr::Index(Box::new(self.alt.clone()), Box::new(self.pri.clone())))),
            "lidx.b" | "idxaddr.b" | "idxaddr.p.b" | "lidx.p.b" => self.pri = Expr::Index(Box::new(self.alt.clone()), Box::new(self.pri.clone())),
            "move.pri" => self.pri = self.alt.clone(),
            "move.alt" => self.alt = self.pri.clone(),
            "xchg" => std::mem::swap(&mut self.pri, &mut self.alt),
            "swap.pri" => {
                let top = self.pop();
                self.stack.push(std::mem::replace(&mut self.pri, top));
            }
            "swap.alt" => {
                let top = self.pop();
                self.stack.push(std::mem::replace(&mut self.alt, top));
            }
            "push.pri" | "pushr.pri" => self.stack.push(self.pri.clone()),
            "push.alt" => self.stack.push(self.alt.clone()),
            "push.c" | "push.p.c" | "pushr.c" | "pushr.p.c" => self.stack.push(Expr::Number(first)),
            "push" | "push.p" => self.stack.push(Expr::Data(first / 4)),
            "push.s" | "push.p.s" | "pushr.s" | "pushr.p.s" => self.stack.push(variable(first)),
            "push.adr" | "push.p.adr" | "pushr.adr" | "pushr.p.adr" => self.stack.push(Expr::Address(Box::new(variable(first)))),
            "push2.c" | "push3.c" | "push4.c" | "push5.c" => c.iter().for_each(|v| self.stack.push(Expr::Number(*v))),
            "push2" | "push3" | "push4" | "push5" => c.iter().for_each(|v| self.stack.push(Expr::Data(*v / 4))),
            "push2.s" | "push3.s" | "push4.s" | "push5.s" => c.iter().for_each(|v| self.stack.push(variable(*v))),
            "push2.adr" | "push3.adr" | "push4.adr" | "push5.adr" => c.iter().for_each(|v| self.stack.push(Expr::Address(Box::new(variable(*v))))),
            "pop.pri" => self.pri = self.pop(),
            "pop.alt" => self.alt = self.pop(),
            "stack" | "stack.p" => {
                // A positive value gives back the room of the parameters after a call.
                if first > 0 {
                    for _ in 0..first / 4 {
                        self.stack.pop();
                    }
                }
            }
            "heap" | "heap.p" => {}
            "add" => self.binary("+"),
            "sub" => self.binary("-"),
            "sub.alt" => {
                std::mem::swap(&mut self.pri, &mut self.alt);
                self.binary("-");
            }
            "smul" | "umul" => self.binary("*"),
            "sdiv" | "udiv" => {
                let divisor = self.alt.clone();
                let value = self.pri.clone();
                self.pri = Expr::Binary(Box::new(value.clone()), "/", Box::new(divisor.clone()));
                self.alt = Expr::Binary(Box::new(value), "%", Box::new(divisor));
            }
            "sdiv.alt" | "udiv.alt" => {
                let divisor = self.pri.clone();
                let value = self.alt.clone();
                self.pri = Expr::Binary(Box::new(value.clone()), "/", Box::new(divisor.clone()));
                self.alt = Expr::Binary(Box::new(value), "%", Box::new(divisor));
            }
            "shl" => self.binary("<<"),
            "shr" | "sshr" => self.binary(">>"),
            "and" => self.binary("&"),
            "or" => self.binary("|"),
            "xor" => self.binary("^"),
            "not" => self.pri = self.pri.negated(),
            "neg" => self.pri = Expr::Unary("-", Box::new(self.pri.clone())),
            "invert" => self.pri = Expr::Unary("~", Box::new(self.pri.clone())),
            "add.c" | "add.p.c" => self.pri = Expr::Binary(Box::new(self.pri.clone()), "+", Box::new(Expr::Number(first))),
            "smul.c" | "smul.p.c" => self.pri = Expr::Binary(Box::new(self.pri.clone()), "*", Box::new(Expr::Number(first))),
            "shl.c.pri" | "shl.p.c.pri" => self.pri = Expr::Binary(Box::new(self.pri.clone()), "<<", Box::new(Expr::Number(first))),
            "shl.c.alt" | "shl.p.c.alt" => self.alt = Expr::Binary(Box::new(self.alt.clone()), "<<", Box::new(Expr::Number(first))),
            "shr.c.pri" | "shr.p.c.pri" => self.pri = Expr::Binary(Box::new(self.pri.clone()), ">>", Box::new(Expr::Number(first))),
            "shr.c.alt" | "shr.p.c.alt" => self.alt = Expr::Binary(Box::new(self.alt.clone()), ">>", Box::new(Expr::Number(first))),
            "zero.pri" => self.pri = Expr::Number(0),
            "zero.alt" => self.alt = Expr::Number(0),
            "zero" | "zero.p" => self.out.push(Stmt::Assign(Expr::Data(first / 4), Expr::Number(0))),
            "zero.s" | "zero.p.s" => self.out.push(Stmt::Assign(variable(first), Expr::Number(0))),
            "sign.pri" | "sign.alt" => {}
            "eq" => self.compare("==", None),
            "neq" => self.compare("!=", None),
            "less" | "sless" => self.compare("<", None),
            "leq" | "sleq" => self.compare("<=", None),
            "grtr" | "sgrtr" => self.compare(">", None),
            "geq" | "sgeq" => self.compare(">=", None),
            "eq.c.pri" | "eq.p.c.pri" => self.compare("==", Some(first)),
            "eq.c.alt" | "eq.p.c.alt" => {
                self.pri = self.alt.clone();
                self.compare("==", Some(first));
            }
            "inc.pri" => self.pri = Expr::Binary(Box::new(self.pri.clone()), "+", Box::new(Expr::Number(1))),
            "inc.alt" => self.alt = Expr::Binary(Box::new(self.alt.clone()), "+", Box::new(Expr::Number(1))),
            "dec.pri" => self.pri = Expr::Binary(Box::new(self.pri.clone()), "-", Box::new(Expr::Number(1))),
            "dec.alt" => self.alt = Expr::Binary(Box::new(self.alt.clone()), "-", Box::new(Expr::Number(1))),
            "inc" | "inc.p" => self.out.push(Stmt::Assign(Expr::Data(first / 4), Expr::Binary(Box::new(Expr::Data(first / 4)), "+", Box::new(Expr::Number(1))))),
            "dec" | "dec.p" => self.out.push(Stmt::Assign(Expr::Data(first / 4), Expr::Binary(Box::new(Expr::Data(first / 4)), "-", Box::new(Expr::Number(1))))),
            "inc.s" | "inc.p.s" => self.out.push(Stmt::Assign(variable(first), Expr::Binary(Box::new(variable(first)), "+", Box::new(Expr::Number(1))))),
            "dec.s" | "dec.p.s" => self.out.push(Stmt::Assign(variable(first), Expr::Binary(Box::new(variable(first)), "-", Box::new(Expr::Number(1))))),
            "inc.i" => self.out.push(Stmt::Assign(Expr::Deref(Box::new(self.pri.clone())), Expr::Binary(Box::new(Expr::Deref(Box::new(self.pri.clone()))), "+", Box::new(Expr::Number(1))))),
            "dec.i" => self.out.push(Stmt::Assign(Expr::Deref(Box::new(self.pri.clone())), Expr::Binary(Box::new(Expr::Deref(Box::new(self.pri.clone()))), "-", Box::new(Expr::Number(1))))),
            "call" => {
                let target = target_of(at.at + 4, first);
                let count = match self.pop() {
                    Expr::Number(bytes) => (bytes / 4).max(0) as usize,
                    _ => 0,
                };
                let name = self.functions.get(&target).cloned().unwrap_or_else(|| format!("func_{target:04X}"));
                self.call(name, count);
            }
            "sysreq.n" => {
                let name = (self.natives)(first);
                let count = (c[1] / 4).max(0) as usize;
                self.call(name, count);
            }
            "sysreq.c" => {
                let name = (self.natives)(first);
                let count = match self.pop() {
                    Expr::Number(bytes) => (bytes / 4).max(0) as usize,
                    _ => 0,
                };
                self.call(name, count);
            }
            "retn" | "ret" => {
                let value = if matches!(self.pri, Expr::Unknown) { None } else { Some(self.pri.clone()) };
                self.out.push(Stmt::Return(value));
            }
            "halt" | "halt.p" => self.out.push(Stmt::Note("halt".to_string())),
            "jump" => self.out.push(Stmt::Goto(target_of(at.at + 4, first))),
            "jzer" => self.branch(at, first, self.pri.negated()),
            "jnz" => self.branch(at, first, self.pri.clone()),
            "jeq" => self.branch(at, first, Expr::Binary(Box::new(self.pri.clone()), "==", Box::new(self.alt.clone()))),
            "jneq" => self.branch(at, first, Expr::Binary(Box::new(self.pri.clone()), "!=", Box::new(self.alt.clone()))),
            "jless" | "jsless" => self.branch(at, first, Expr::Binary(Box::new(self.pri.clone()), "<", Box::new(self.alt.clone()))),
            "jleq" | "jsleq" => self.branch(at, first, Expr::Binary(Box::new(self.pri.clone()), "<=", Box::new(self.alt.clone()))),
            "jgrtr" | "jsgrtr" => self.branch(at, first, Expr::Binary(Box::new(self.pri.clone()), ">", Box::new(self.alt.clone()))),
            "jgeq" | "jsgeq" => self.branch(at, first, Expr::Binary(Box::new(self.pri.clone()), ">=", Box::new(self.alt.clone()))),
            "switch" => {}
            "casetbl" => {
                let count = first as usize;
                let default = target_of(at.at + 8, c[1]);
                let cases = (0..count).map(|k| (c[2 + 2 * k], target_of(at.at + 16 + 8 * k, c[3 + 2 * k]))).collect();
                self.out.push(Stmt::Switch(self.pri.clone(), cases, default));
            }
            _ => {
                self.out.push(Stmt::Note(format!("{name} {}", c.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))));
            }
        }
    }
}

/// A result that is used once, in the next statement, goes into that statement; one that is not used loses its name.
fn simplify(stmts: Vec<Stmt>) -> Vec<Stmt> {
    let mut list = stmts;
    let mut at = 0;
    while at < list.len() {
        if let Stmt::Let(id, value) = list[at].clone() {
            let mut uses = 0;
            let mut last = None;
            for (k, s) in list.iter().enumerate().skip(at + 1) {
                let n = stmt_uses(s, id);
                if n > 0 {
                    uses += n;
                    last = Some(k);
                }
            }
            match (uses, last) {
                (0, _) => list[at] = Stmt::Run(value),
                (1, Some(k)) if k == at + 1 && !matches!(list[k], Stmt::Label(_)) => {
                    list[k] = replace_in(&list[k], id, &value);
                    list.remove(at);
                    continue;
                }
                _ => {}
            }
        }
        at += 1;
    }
    list
}

fn stmt_uses(s: &Stmt, id: u32) -> usize {
    match s {
        Stmt::Assign(a, b) => a.uses_result(id) + b.uses_result(id),
        Stmt::Let(_, e) | Stmt::Run(e) | Stmt::Branch(e, _) | Stmt::Switch(e, _, _) => e.uses_result(id),
        Stmt::Return(Some(e)) => e.uses_result(id),
        _ => 0,
    }
}

fn replace_in(s: &Stmt, id: u32, with: &Expr) -> Stmt {
    match s {
        Stmt::Assign(a, b) => Stmt::Assign(a.replace_result(id, with), b.replace_result(id, with)),
        Stmt::Let(n, e) => Stmt::Let(*n, e.replace_result(id, with)),
        Stmt::Run(e) => Stmt::Run(e.replace_result(id, with)),
        Stmt::Branch(e, l) => Stmt::Branch(e.replace_result(id, with), *l),
        Stmt::Switch(e, c, d) => Stmt::Switch(e.replace_result(id, with), c.clone(), *d),
        Stmt::Return(Some(e)) => Stmt::Return(Some(e.replace_result(id, with))),
        other => other.clone(),
    }
}

/// Writes the statements with `if`, `if else` and `while` where the jumps have a simple shape.
fn structure(list: &[Stmt], used: &HashMap<i64, usize>, depth: usize, out: &mut Vec<String>) {
    let pad = "    ".repeat(depth);
    let position = |label: i64, from: usize| list.iter().enumerate().skip(from).find(|(_, s)| matches!(s, Stmt::Label(l) if *l == label)).map(|(k, _)| k);
    let mut at = 0;
    while at < list.len() {
        match &list[at] {
            Stmt::Label(l) => {
                // A loop: the label is jumped to again from the end of the statements that follow.
                let back = list.iter().enumerate().skip(at + 1).rev().find(|(_, s)| matches!(s, Stmt::Goto(g) if g == l)).map(|(k, _)| k);
                if let (Some(end), Some(Stmt::Branch(cond, exit))) = (back, list.get(at + 1)) {
                    if position(*exit, at + 1) == Some(end + 1) {
                        out.push(format!("{pad}while ({}) {{", cond.negated().text()));
                        structure(&list[at + 2..end], used, depth + 1, out);
                        out.push(format!("{pad}}}"));
                        at = end + 2;
                        continue;
                    }
                }
                if used.get(l).copied().unwrap_or(0) > 0 {
                    out.push(format!("{}L{l:04X}:", "    ".repeat(depth.saturating_sub(1))));
                }
            }
            Stmt::Branch(cond, label) => {
                if let Some(end) = position(*label, at + 1) {
                    // if (...) { ... } or if (...) { ... } else { ... }
                    let body_end = end;
                    let mut else_end = None;
                    if let Some(Stmt::Goto(skip)) = list.get(body_end.wrapping_sub(1)).filter(|_| body_end > at + 1) {
                        if let Some(k) = position(*skip, end + 1) {
                            else_end = Some((body_end - 1, k));
                        }
                    }
                    out.push(format!("{pad}if ({}) {{", cond.negated().text()));
                    match else_end {
                        Some((then_end, k)) => {
                            structure(&list[at + 1..then_end], used, depth + 1, out);
                            out.push(format!("{pad}}} else {{"));
                            structure(&list[end + 1..k], used, depth + 1, out);
                            out.push(format!("{pad}}}"));
                            at = k + 1;
                        }
                        None => {
                            structure(&list[at + 1..body_end], used, depth + 1, out);
                            out.push(format!("{pad}}}"));
                            at = end + 1;
                        }
                    }
                    continue;
                }
                out.push(format!("{pad}if ({}) goto L{label:04X};", cond.text()));
            }
            Stmt::Goto(l) => out.push(format!("{pad}goto L{l:04X};")),
            Stmt::Assign(a, b) => out.push(format!("{pad}{} = {};", a.text(), b.text())),
            Stmt::Let(n, e) => out.push(format!("{pad}result{n} = {};", e.text())),
            Stmt::Run(e) => out.push(format!("{pad}{};", e.text())),
            Stmt::Return(None) => out.push(format!("{pad}return;")),
            Stmt::Return(Some(e)) => out.push(format!("{pad}return {};", e.text())),
            Stmt::Switch(e, cases, default) => {
                out.push(format!("{pad}switch ({}) {{", e.text()));
                for (value, label) in cases {
                    out.push(format!("{pad}    case {value}: goto L{label:04X};"));
                }
                out.push(format!("{pad}    default: goto L{default:04X};"));
                out.push(format!("{pad}}}"));
            }
            Stmt::Note(text) => out.push(format!("{pad}// {text}")),
        }
        at += 1;
    }
}

/// The code of a script as Pawn-like text. `native_name` gives the name of a native from its number in the table of the file.
pub fn to_pawn(amx: &Amx, native_name: &dyn Fn(u32) -> Option<String>) -> Result<String, String> {
    let code = decode(&amx.code)?;
    let natives = |index: i32| -> String {
        amx.natives.get(index as usize).map_or_else(|| format!("native_{index}"), |n| native_name(n.name).unwrap_or_else(|| format!("native_{:08X}", n.name)))
    };
    // The functions: where a `proc` is. A public function is named by its number.
    let mut functions: BTreeMap<i64, String> = BTreeMap::new();
    for instruction in &code {
        if opcode(instruction.number).is_some_and(|(n, _)| n == "proc") {
            functions.insert(instruction.at as i64, format!("func_{:04X}", instruction.at));
        }
    }
    for entry in &amx.publics {
        functions.insert(i64::from(entry.address), format!("public_{:08X}", entry.name));
    }
    let starts: Vec<usize> = functions.keys().map(|&a| a as usize).collect();
    let mut text = String::new();
    text.push_str("// Pawn-like text made from the code, to read. It is a guess: the names are made up and the shape of the structures may be wrong.\n");
    text.push_str("// Change the script in the Assembly tab.\n\n");
    for (place, &start) in starts.iter().enumerate() {
        let end = starts.get(place + 1).copied().unwrap_or(usize::MAX);
        let body: Vec<&Instruction> = code.iter().filter(|i| i.at >= start && i.at < end).collect();
        let mut machine = Machine { pri: Expr::Unknown, alt: Expr::Unknown, stack: Vec::new(), out: Vec::new(), results: 0, natives: &natives, functions: &functions };
        let targets: BTreeSet<i64> = body
            .iter()
            .flat_map(|i| match opcode(i.number).map(|(_, f)| f) {
                Some(Form::Jump) if opcode(i.number).is_some_and(|(n, _)| n != "call") => vec![target_of(i.at + 4, i.cells[0])],
                Some(Form::CaseTable) => std::iter::once(target_of(i.at + 8, i.cells[1])).chain((0..i.cells[0] as usize).map(|k| target_of(i.at + 16 + 8 * k, i.cells[3 + 2 * k]))).collect(),
                _ => Vec::new(),
            })
            .collect();
        for instruction in &body {
            if targets.contains(&(instruction.at as i64)) {
                // The registers are not known after a label that is jumped to.
                machine.out.push(Stmt::Label(instruction.at as i64));
                machine.pri = Expr::Unknown;
                machine.alt = Expr::Unknown;
            }
            machine.step(instruction);
        }
        let statements = simplify(machine.out);
        let mut used: HashMap<i64, usize> = HashMap::new();
        for s in &statements {
            match s {
                Stmt::Goto(l) | Stmt::Branch(_, l) => *used.entry(*l).or_default() += 1,
                Stmt::Switch(_, cases, d) => {
                    *used.entry(*d).or_default() += 1;
                    cases.iter().for_each(|(_, l)| *used.entry(*l).or_default() += 1);
                }
                _ => {}
            }
        }
        let name = functions.get(&(start as i64)).cloned().unwrap_or_default();
        let max_arg = statements_max_arg(&statements);
        text.push_str(&format!("{name}({}) {{\n", (1..=max_arg).map(|k| format!("arg{k}")).collect::<Vec<_>>().join(", ")));
        let mut lines = Vec::new();
        structure(&statements, &used, 1, &mut lines);
        for line in lines {
            text.push_str(&line);
            text.push('\n');
        }
        text.push_str("}\n\n");
    }
    Ok(text)
}

fn statements_max_arg(list: &[Stmt]) -> i32 {
    fn expr(e: &Expr, best: &mut i32) {
        match e {
            Expr::Arg(i) => *best = (*best).max(*i),
            Expr::Binary(a, _, b) | Expr::Index(a, b) => {
                expr(a, best);
                expr(b, best);
            }
            Expr::Unary(_, a) | Expr::Address(a) | Expr::Deref(a) => expr(a, best),
            Expr::Call(_, args) => args.iter().for_each(|a| expr(a, best)),
            _ => {}
        }
    }
    let mut best = 0;
    for s in list {
        match s {
            Stmt::Assign(a, b) => {
                expr(a, &mut best);
                expr(b, &mut best);
            }
            Stmt::Let(_, e) | Stmt::Run(e) | Stmt::Branch(e, _) | Stmt::Switch(e, _, _) | Stmt::Return(Some(e)) => expr(e, &mut best),
            _ => {}
        }
    }
    best
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_condition_is_turned_around() {
        let a = Expr::Binary(Box::new(Expr::Local(1)), "<", Box::new(Expr::Number(5)));
        assert_eq!(a.negated().text(), "local1 >= 5");
        assert_eq!(Expr::Local(1).negated().text(), "!local1");
        assert_eq!(Expr::Local(1).negated().negated().text(), "local1");
    }

    /// With the dump: every script of the game is turned into text, and the text is not empty.
    #[test]
    fn the_scripts_of_the_game_are_turned_into_text() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let names: std::collections::HashMap<u32, String> = crate::natives::NATIVES.iter().map(|n| (crate::natives::hash(n.name), n.name.to_string())).collect();
        let (mut scripts, mut lines, mut calls) = (0, 0, 0);
        for archive in [7usize, 29, 89] {
            let garc = sango_core::Garc::open(&dump.archive(archive)).unwrap();
            for id in 0..garc.file_count() {
                let Some(bytes) = garc.first_file_decompressed(id) else { continue };
                if bytes.len() < 8 || u16::from_le_bytes([bytes[4], bytes[5]]) != crate::amx::MAGIC {
                    continue;
                }
                let amx = Amx::parse(&bytes).unwrap();
                let text = to_pawn(&amx, &|h| names.get(&h).cloned()).unwrap_or_else(|e| panic!("archive {archive} file {id}: {e}"));
                scripts += 1;
                lines += text.lines().count();
                calls += text.matches("WorkGet(").count() + text.matches("FlagGet(").count() + text.matches("MsgWin").count();
            }
        }
        assert!(scripts >= 90 && lines > 10_000, "{scripts} scripts, {lines} lines");
        assert!(calls > 100, "the natives are named in the calls: {calls}");
        eprintln!("{scripts} scripts, {lines} lines of Pawn-like text");
    }
}
