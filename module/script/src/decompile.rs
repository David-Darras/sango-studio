//! The code of a script as Pawn-like text, for reading. It is made by following the stack machine of the code: the values that are pushed
//! become the parameters of the calls, the jumps become `if` and `while` when their shape is simple, and the rest stays as `goto`.
//!
//! The text is a help to understand the assembly, not a program that can be built again: the shape of the structures is a guess. The names are
//! found where they can be:
//!
//! - a function that only calls one other function (a native, or a function of the script) has the name of that function and a number: a
//!   function that only calls `set_flag` is `set_flag_0`, the next one `set_flag_1`;
//! - an argument or a local that goes into a native has the name of that parameter of the native (`character`, `flag`...), and the name goes
//!   on through the calls of the functions of the script and through the copies (`local2 = arg1`) until nothing changes;
//! - the other variables keep a made-up name (`arg1`, `local1`, `data3`).

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
    /// A variable with a name that was found (see the top of the file).
    Name(String),
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
            Expr::Name(name) => name.clone(),
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
    /// The address of the instruction of each statement of `out`, and of the instruction that runs now.
    addrs: Vec<usize>,
    here: usize,
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
    /// Adds a statement, with the address of the instruction that runs now.
    fn emit(&mut self, statement: Stmt) {
        self.out.push(statement);
        self.addrs.push(self.here);
    }

    fn pop(&mut self) -> Expr {
        self.stack.pop().unwrap_or(Expr::Unknown)
    }

    fn call(&mut self, name: String, count: usize) {
        let args: Vec<Expr> = (0..count).map(|_| self.pop()).collect();
        self.results += 1;
        let id = self.results;
        self.emit(Stmt::Let(id, Expr::Call(name, args)));
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
        self.emit(Stmt::Branch(jump_when, target_of(at.at + 4, cell)));
    }

    /// Runs one instruction.
    fn step(&mut self, at: &Instruction) {
        let Some((name, form)) = opcode(at.number) else { return };
        let c = &at.cells;
        // A value packed in the opcode has 16 bits and a sign: a place of the stack is often negative.
        let first = c.first().copied().map_or(0, |v| if form == Form::Packed { i32::from(v as u16 as i16) } else { v });
        match name {
            "proc" | "break" | "nop" | "align.pri" | "align.alt" => {}
            "const" => self.emit(Stmt::Assign(Expr::Data(first / 4), Expr::Number(c[1]))),
            "const.s" => self.emit(Stmt::Assign(variable(first), Expr::Number(c[1]))),
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
            "stor.pri" | "stor.p.pri" => self.emit(Stmt::Assign(Expr::Data(first / 4), self.pri.clone())),
            "stor.alt" | "stor.p.alt" => self.emit(Stmt::Assign(Expr::Data(first / 4), self.alt.clone())),
            "stor.s.pri" | "stor.p.s.pri" => self.emit(Stmt::Assign(variable(first), self.pri.clone())),
            "stor.s.alt" | "stor.p.s.alt" => self.emit(Stmt::Assign(variable(first), self.alt.clone())),
            "sref.s.pri" | "sref.p.s.pri" => self.emit(Stmt::Assign(Expr::Deref(Box::new(variable(first))), self.pri.clone())),
            "sref.s.alt" | "sref.p.s.alt" => self.emit(Stmt::Assign(Expr::Deref(Box::new(variable(first))), self.alt.clone())),
            "sref.pri" | "sref.p.pri" => self.emit(Stmt::Assign(Expr::Deref(Box::new(Expr::Data(first / 4))), self.pri.clone())),
            "sref.alt" | "sref.p.alt" => self.emit(Stmt::Assign(Expr::Deref(Box::new(Expr::Data(first / 4))), self.alt.clone())),
            "stor.i" => self.emit(Stmt::Assign(Expr::Deref(Box::new(self.alt.clone())), self.pri.clone())),
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
            "zero" | "zero.p" => self.emit(Stmt::Assign(Expr::Data(first / 4), Expr::Number(0))),
            "zero.s" | "zero.p.s" => self.emit(Stmt::Assign(variable(first), Expr::Number(0))),
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
            "inc" | "inc.p" => self.emit(Stmt::Assign(Expr::Data(first / 4), Expr::Binary(Box::new(Expr::Data(first / 4)), "+", Box::new(Expr::Number(1))))),
            "dec" | "dec.p" => self.emit(Stmt::Assign(Expr::Data(first / 4), Expr::Binary(Box::new(Expr::Data(first / 4)), "-", Box::new(Expr::Number(1))))),
            "inc.s" | "inc.p.s" => self.emit(Stmt::Assign(variable(first), Expr::Binary(Box::new(variable(first)), "+", Box::new(Expr::Number(1))))),
            "dec.s" | "dec.p.s" => self.emit(Stmt::Assign(variable(first), Expr::Binary(Box::new(variable(first)), "-", Box::new(Expr::Number(1))))),
            "inc.i" => self.emit(Stmt::Assign(Expr::Deref(Box::new(self.pri.clone())), Expr::Binary(Box::new(Expr::Deref(Box::new(self.pri.clone()))), "+", Box::new(Expr::Number(1))))),
            "dec.i" => self.emit(Stmt::Assign(Expr::Deref(Box::new(self.pri.clone())), Expr::Binary(Box::new(Expr::Deref(Box::new(self.pri.clone()))), "-", Box::new(Expr::Number(1))))),
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
                self.emit(Stmt::Return(value));
            }
            "halt" | "halt.p" => self.emit(Stmt::Note("halt".to_string())),
            "jump" => self.emit(Stmt::Goto(target_of(at.at + 4, first))),
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
                self.emit(Stmt::Switch(self.pri.clone(), cases, default));
            }
            _ => {
                self.emit(Stmt::Note(format!("{name} {}", c.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))));
            }
        }
    }
}

/// A result that is used once, in the next statement, goes into that statement; one that is not used loses its name.
fn simplify(stmts: Vec<Stmt>, addrs: Vec<usize>) -> (Vec<Stmt>, Vec<usize>) {
    let mut list = stmts;
    let mut addrs = addrs;
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
                    // The line starts at the first instruction of the value.
                    addrs[k] = addrs[at];
                    list.remove(at);
                    addrs.remove(at);
                    continue;
                }
                _ => {}
            }
        }
        at += 1;
    }
    (list, addrs)
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
fn structure(list: &[Stmt], addrs: &[usize], used: &HashMap<i64, usize>, depth: usize, out: &mut Vec<(String, usize)>) {
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
                        out.push((format!("{pad}while ({}) {{", cond.negated().text()), addrs[at]));
                        structure(&list[at + 2..end], &addrs[at + 2..end], used, depth + 1, out);
                        out.push((format!("{pad}}}"), addrs[at]));
                        at = end + 2;
                        continue;
                    }
                }
                if used.get(l).copied().unwrap_or(0) > 0 {
                    out.push((format!("{}L{l:04X}:", "    ".repeat(depth.saturating_sub(1))), addrs[at]));
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
                    out.push((format!("{pad}if ({}) {{", cond.negated().text()), addrs[at]));
                    match else_end {
                        Some((then_end, k)) => {
                            structure(&list[at + 1..then_end], &addrs[at + 1..then_end], used, depth + 1, out);
                            out.push((format!("{pad}}} else {{"), addrs[at]));
                            structure(&list[end + 1..k], &addrs[end + 1..k], used, depth + 1, out);
                            out.push((format!("{pad}}}"), addrs[at]));
                            at = k + 1;
                        }
                        None => {
                            structure(&list[at + 1..body_end], &addrs[at + 1..body_end], used, depth + 1, out);
                            out.push((format!("{pad}}}"), addrs[at]));
                            at = end + 1;
                        }
                    }
                    continue;
                }
                out.push((format!("{pad}if ({}) goto L{label:04X};", cond.text()), addrs[at]));
            }
            Stmt::Goto(l) => out.push((format!("{pad}goto L{l:04X};"), addrs[at])),
            Stmt::Assign(a, b) => out.push((format!("{pad}{} = {};", a.text(), b.text()), addrs[at])),
            Stmt::Let(n, e) => out.push((format!("{pad}result{n} = {};", e.text()), addrs[at])),
            Stmt::Run(e) => out.push((format!("{pad}{};", e.text()), addrs[at])),
            Stmt::Return(None) => out.push((format!("{pad}return;"), addrs[at])),
            Stmt::Return(Some(e)) => out.push((format!("{pad}return {};", e.text()), addrs[at])),
            Stmt::Switch(e, cases, default) => {
                out.push((format!("{pad}switch ({}) {{", e.text()), addrs[at]));
                for (value, label) in cases {
                    out.push((format!("{pad}    case {value}: goto L{label:04X};"), addrs[at]));
                }
                out.push((format!("{pad}    default: goto L{default:04X};"), addrs[at]));
                out.push((format!("{pad}}}"), addrs[at]));
            }
            Stmt::Note(text) => out.push((format!("{pad}// {text}"), addrs[at])),
        }
        at += 1;
    }
}

/// A variable of a function that can get a name: an argument or a local, by its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Var {
    Arg(i32),
    Local(i32),
}

/// The variable that an expression is, or whose address or content it is.
fn var_of(e: &Expr) -> Option<Var> {
    match e {
        Expr::Arg(k) => Some(Var::Arg(*k)),
        Expr::Local(k) => Some(Var::Local(*k)),
        Expr::Address(inner) | Expr::Deref(inner) => var_of(inner),
        _ => None,
    }
}

/// Gives every call of an expression to `found`.
fn calls_in<'a>(e: &'a Expr, found: &mut Vec<(&'a str, &'a [Expr])>) {
    match e {
        Expr::Call(name, args) => {
            found.push((name.as_str(), args.as_slice()));
            args.iter().for_each(|a| calls_in(a, found));
        }
        Expr::Binary(a, _, b) | Expr::Index(a, b) => {
            calls_in(a, found);
            calls_in(b, found);
        }
        Expr::Unary(_, a) | Expr::Address(a) | Expr::Deref(a) => calls_in(a, found),
        _ => {}
    }
}

/// The expressions of a statement.
fn exprs_of(s: &Stmt) -> Vec<&Expr> {
    match s {
        Stmt::Assign(a, b) => vec![a, b],
        Stmt::Let(_, e) | Stmt::Run(e) | Stmt::Branch(e, _) | Stmt::Switch(e, _, _) | Stmt::Return(Some(e)) => vec![e],
        _ => Vec::new(),
    }
}

/// The calls of a list of statements.
fn calls_of(list: &[Stmt]) -> Vec<(&str, &[Expr])> {
    let mut found = Vec::new();
    for s in list {
        for e in exprs_of(s) {
            calls_in(e, &mut found);
        }
    }
    found
}

/// The function that a function only calls, when it does nothing else: no jump, no change of a variable, one call, and a return of its value
/// or of nothing.
fn only_call(list: &[Stmt]) -> Option<&str> {
    let busy = list.iter().any(|s| matches!(s, Stmt::Assign(..) | Stmt::Branch(..) | Stmt::Switch(..) | Stmt::Goto(_)));
    let calls = calls_of(list);
    (!busy && calls.len() == 1).then(|| calls[0].0)
}

/// The renaming of the functions and of the variables (see the top of the file). `functions` gives the statements of each function by its name;
/// `params` gives the names of the parameters of a native from its name in the text. Returns the new name of each function that gets one, and the
/// names of the variables of each function.
fn find_names(functions: &[(String, Vec<Stmt>)], params: &dyn Fn(&str) -> Option<Vec<String>>) -> (HashMap<String, String>, HashMap<String, BTreeMap<Var, String>>) {
    // The functions that only call one other function. A function that calls such a function takes the base of its name.
    let mut renamed: HashMap<String, String> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (name, list) in functions {
        if name.starts_with("public_") {
            continue;
        }
        if let Some(callee) = only_call(list) {
            let base = renamed.get(callee).map_or(callee, |n| n.rsplit_once('_').map_or(n.as_str(), |(base, _)| base)).to_string();
            let number = counts.entry(base.clone()).or_default();
            renamed.insert(name.clone(), format!("{base}_{number}"));
            *number += 1;
        }
    }
    // The names of the variables: from the parameters of the natives, then through the calls and the copies, until nothing changes.
    let mut names: HashMap<String, BTreeMap<Var, String>> = functions.iter().map(|(n, _)| (n.clone(), BTreeMap::new())).collect();
    for _ in 0..12 {
        let mut changed = false;
        for (name, list) in functions {
            let mut found: Vec<(Var, String)> = Vec::new();
            for (callee, args) in calls_of(list) {
                let callee_names: Option<Vec<String>> = params(callee).or_else(|| {
                    names.get(callee).map(|vars| (1..=args.len() as i32).map(|k| vars.get(&Var::Arg(k)).cloned().unwrap_or_default()).collect())
                });
                let Some(callee_names) = callee_names else { continue };
                for (arg, param) in args.iter().zip(callee_names) {
                    if let (Some(var), false) = (var_of(arg), param.is_empty()) {
                        found.push((var, param));
                    }
                }
            }
            // The copies: a variable that gets a named variable (or gives its value to one) has the same name.
            for s in list {
                if let Stmt::Assign(a, b) = s {
                    if let (Some(x), Some(y)) = (var_of(a), var_of(b)) {
                        let vars = &names[name];
                        if let Some(n) = vars.get(&y).cloned() {
                            found.push((x, n));
                        } else if let Some(n) = vars.get(&x).cloned() {
                            found.push((y, n));
                        }
                    }
                }
            }
            let vars = names.get_mut(name).expect("every function has its map");
            for (var, param) in found {
                if !vars.contains_key(&var) {
                    vars.insert(var, param);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    (renamed, names)
}

/// The text of a variable in a function: its name, made unique in the function (`flag`, `flag_2`), or its made-up name.
fn display_names(vars: &BTreeMap<Var, String>) -> HashMap<Var, String> {
    let mut out = HashMap::new();
    let mut taken: HashMap<String, usize> = HashMap::new();
    for (var, name) in vars {
        let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
        let count = taken.entry(clean.clone()).or_default();
        *count += 1;
        out.insert(*var, if *count == 1 { clean } else { format!("{clean}_{count}") });
    }
    out
}

impl Expr {
    /// The expression with the names of the variables and of the functions.
    fn named(&self, vars: &HashMap<Var, String>, functions: &HashMap<String, String>) -> Expr {
        let go = |e: &Expr| Box::new(e.named(vars, functions));
        match self {
            Expr::Arg(k) => vars.get(&Var::Arg(*k)).map_or_else(|| self.clone(), |n| Expr::Name(n.clone())),
            Expr::Local(k) => vars.get(&Var::Local(*k)).map_or_else(|| self.clone(), |n| Expr::Name(n.clone())),
            Expr::Binary(a, op, b) => Expr::Binary(go(a), op, go(b)),
            Expr::Index(a, b) => Expr::Index(go(a), go(b)),
            Expr::Unary(op, a) => Expr::Unary(op, go(a)),
            Expr::Address(a) => Expr::Address(go(a)),
            Expr::Deref(a) => Expr::Deref(go(a)),
            Expr::Call(name, args) => Expr::Call(functions.get(name).cloned().unwrap_or_else(|| name.clone()), args.iter().map(|a| a.named(vars, functions)).collect()),
            other => other.clone(),
        }
    }
}

/// The statement with the names of the variables and of the functions.
fn named_stmt(s: &Stmt, vars: &HashMap<Var, String>, functions: &HashMap<String, String>) -> Stmt {
    let n = |e: &Expr| e.named(vars, functions);
    match s {
        Stmt::Assign(a, b) => Stmt::Assign(n(a), n(b)),
        Stmt::Let(id, e) => Stmt::Let(*id, n(e)),
        Stmt::Run(e) => Stmt::Run(n(e)),
        Stmt::Branch(e, l) => Stmt::Branch(n(e), *l),
        Stmt::Switch(e, c, d) => Stmt::Switch(n(e), c.clone(), *d),
        Stmt::Return(Some(e)) => Stmt::Return(Some(n(e))),
        other => other.clone(),
    }
}

/// The names that the Pawn-like text gives, for the assembly: the name of each function by the address of its first instruction, and the
/// name of each variable of a function by the place of the variable on the stack (an argument at 12, 16..., a local at -4, -8...).
pub struct Names {
    pub functions: BTreeMap<usize, String>,
    pub variables: BTreeMap<usize, HashMap<i32, String>>,
}

/// The names of the functions and of the variables of a script (see [`Names`]).
pub fn names(amx: &Amx, native_name: &dyn Fn(u32) -> Option<String>) -> Result<Names, String> {
    let (bodies, body_addrs) = bodies(amx, native_name)?;
    let starts = body_addrs.iter().map(|(start, _)| *start);
    let params = |name: &str| crate::natives::find(name).filter(|n| !n.params.is_empty()).map(|n| n.params.iter().map(|p| p.name.to_string()).collect::<Vec<String>>());
    let (renamed, variables) = find_names(&bodies, &params);
    let mut out = Names { functions: BTreeMap::new(), variables: BTreeMap::new() };
    for ((name, _), start) in bodies.iter().zip(starts) {
        out.functions.insert(start, renamed.get(name).cloned().unwrap_or_else(|| name.clone()));
        let shown = display_names(&variables[name]);
        let by_place = shown.into_iter().map(|(var, text)| (match var { Var::Arg(k) => 12 + 4 * (k - 1), Var::Local(k) => -4 * k }, text)).collect();
        out.variables.insert(start, by_place);
    }
    Ok(out)
}

/// The code of a script as Pawn-like text. `native_name` gives the name of a native from its number in the table of the file.
#[cfg(test)]
pub fn to_pawn(amx: &Amx, native_name: &dyn Fn(u32) -> Option<String>) -> Result<String, String> {
    Ok(to_pawn_lines(amx, native_name)?.into_iter().map(|(line, _)| line + "\n").collect())
}

/// The lines of the Pawn-like text, each with the address of the instruction that it comes from (`None` for a line without instruction: a
/// brace, an empty line, the first comments).
pub fn to_pawn_lines(amx: &Amx, native_name: &dyn Fn(u32) -> Option<String>) -> Result<Vec<(String, Option<usize>)>, String> {
    let mut lines: Vec<(String, Option<usize>)> = vec![
        ("// Pawn-like text made from the code, to read. It is a guess: some names are made up and the shape of the structures may be wrong.".to_string(), None),
        ("// Change the script in the assembly, on the left.".to_string(), None),
        (String::new(), None),
    ];
    let (bodies, body_addrs) = bodies(amx, native_name)?;
    let params = |name: &str| crate::natives::find(name).filter(|n| !n.params.is_empty()).map(|n| n.params.iter().map(|p| p.name.to_string()).collect::<Vec<String>>());
    let (renamed, variables) = find_names(&bodies, &params);
    for ((name, statements), (start, addrs)) in bodies.iter().zip(&body_addrs) {
        let vars = display_names(&variables[name]);
        let statements: Vec<Stmt> = statements.iter().map(|s| named_stmt(s, &vars, &renamed)).collect();
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
        // The arguments: the largest number that the code uses (before the names), with their names.
        let max_arg = statements_max_arg(&bodies.iter().find(|(n, _)| n == name).map(|(_, s)| s.clone()).unwrap_or_default());
        let shown = renamed.get(name).unwrap_or(name);
        let args: Vec<String> = (1..=max_arg).map(|k| vars.get(&Var::Arg(k)).cloned().unwrap_or_else(|| format!("arg{k}"))).collect();
        lines.push((format!("{shown}({}) {{", args.join(", ")), Some(*start)));
        let mut body_lines = Vec::new();
        structure(&statements, addrs, &used, 1, &mut body_lines);
        lines.extend(body_lines.into_iter().map(|(line, at)| (line, Some(at))));
        lines.push(("}".to_string(), None));
        lines.push((String::new(), None));
    }
    Ok(lines)
}

/// The statements of every function of a script with their name, and the first address of each function with the address of the instruction of
/// each statement.
fn bodies(amx: &Amx, native_name: &dyn Fn(u32) -> Option<String>) -> Result<(Vec<(String, Vec<Stmt>)>, Vec<(usize, Vec<usize>)>), String> {
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
    let mut bodies: Vec<(String, Vec<Stmt>)> = Vec::new();
    let mut body_addrs: Vec<(usize, Vec<usize>)> = Vec::new();
    for (place, &start) in starts.iter().enumerate() {
        let end = starts.get(place + 1).copied().unwrap_or(usize::MAX);
        let body: Vec<&Instruction> = code.iter().filter(|i| i.at >= start && i.at < end).collect();
        let mut machine = Machine { pri: Expr::Unknown, alt: Expr::Unknown, stack: Vec::new(), out: Vec::new(), results: 0, addrs: Vec::new(), here: start, natives: &natives, functions: &functions };
        let targets: BTreeSet<i64> = body
            .iter()
            .flat_map(|i| match opcode(i.number).map(|(_, f)| f) {
                Some(Form::Jump) if opcode(i.number).is_some_and(|(n, _)| n != "call") => vec![target_of(i.at + 4, i.cells[0])],
                Some(Form::CaseTable) => std::iter::once(target_of(i.at + 8, i.cells[1])).chain((0..i.cells[0] as usize).map(|k| target_of(i.at + 16 + 8 * k, i.cells[3 + 2 * k]))).collect(),
                _ => Vec::new(),
            })
            .collect();
        for instruction in &body {
            machine.here = instruction.at;
            if targets.contains(&(instruction.at as i64)) {
                // The registers are not known after a label that is jumped to.
                machine.emit(Stmt::Label(instruction.at as i64));
                machine.pri = Expr::Unknown;
                machine.alt = Expr::Unknown;
            }
            machine.step(instruction);
        }
        let name = functions.get(&(start as i64)).cloned().unwrap_or_default();
        let (statements, addrs) = simplify(machine.out, machine.addrs);
        bodies.push((name, statements));
        body_addrs.push((start, addrs));
    }
    Ok((bodies, body_addrs))
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

    /// A function that only calls a native takes its name, and the arguments that go into the native take the names of its parameters; the names go
    /// on to the callers.
    #[test]
    fn the_names_come_from_the_natives() {
        let call = |name: &str, args: Vec<Expr>| Expr::Call(name.to_string(), args);
        let functions = vec![
            ("func_0010".to_string(), vec![Stmt::Return(Some(call("set_flag", vec![Expr::Arg(1), Expr::Arg(2)])))]),
            ("func_0020".to_string(), vec![Stmt::Run(call("set_flag", vec![Expr::Number(3), Expr::Number(1)])), Stmt::Return(None)]),
            ("func_0030".to_string(), vec![Stmt::Assign(Expr::Local(1), Expr::Arg(1)), Stmt::Run(call("func_0010", vec![Expr::Local(1), Expr::Number(1)])), Stmt::Run(call("other", vec![]))]),
        ];
        let params = |name: &str| (name == "set_flag").then(|| vec!["flag".to_string(), "value".to_string()]);
        let (renamed, vars) = find_names(&functions, &params);
        assert_eq!(renamed.get("func_0010").map(String::as_str), Some("set_flag_0"));
        assert_eq!(renamed.get("func_0020").map(String::as_str), Some("set_flag_1"));
        assert!(!renamed.contains_key("func_0030"), "a function that calls two functions keeps its name");
        assert_eq!(vars["func_0010"].get(&Var::Arg(1)).map(String::as_str), Some("flag"));
        assert_eq!(vars["func_0010"].get(&Var::Arg(2)).map(String::as_str), Some("value"));
        // Through the call of func_0010, then through the copy local1 = arg1.
        assert_eq!(vars["func_0030"].get(&Var::Local(1)).map(String::as_str), Some("flag"));
        assert_eq!(vars["func_0030"].get(&Var::Arg(1)).map(String::as_str), Some("flag"));
        let shown = display_names(&vars["func_0030"]);
        assert_eq!(shown.get(&Var::Arg(1)).map(String::as_str), Some("flag"));
        assert_eq!(shown.get(&Var::Local(1)).map(String::as_str), Some("flag_2"));
    }

    /// With the dump: every script of the game is turned into text, and the text is not empty.
    #[test]
    fn the_scripts_of_the_game_are_turned_into_text() {
        let Ok(root) = std::env::var("SANGO_DUMP") else { return };
        let dump = sango_core::Dump::open(&root).unwrap();
        let names = crate::natives::display_names();
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
                calls += ["WorkGet", "FlagGet", "MsgWinOpen"].iter().map(|n| text.matches(&format!("{}(", crate::natives::find(n).map_or(*n, |x| x.alias))).count()).sum::<usize>();
            }
        }
        assert!(scripts >= 90 && lines > 10_000, "{scripts} scripts, {lines} lines");
        assert!(calls > 100, "the natives are named in the calls: {calls}");
        if let Ok(file) = std::env::var("SANGO_PAWN_SAMPLE") {
            let garc = sango_core::Garc::open(&dump.archive(29)).unwrap();
            let amx = Amx::parse(&garc.first_file_decompressed(1).unwrap()).unwrap();
            std::fs::write(file, to_pawn(&amx, &|h| names.get(&h).cloned()).unwrap()).unwrap();
        }
        eprintln!("{scripts} scripts, {lines} lines of Pawn-like text");
    }
}
