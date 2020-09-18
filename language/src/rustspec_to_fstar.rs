use crate::rustspec::*;

use crate::typechecker::{DictEntry, TypeDict};
use core::iter::IntoIterator;
use heck::SnakeCase;
use pretty::RcDoc;
use regex::Regex;
use rustc_ast::ast::BinOpKind;
use rustc_session::Session;
use std::fs::File;
use std::io::Write;
use std::path;

const SEQ_MODULE: &'static str = "seq";

fn make_let_binding<'a>(
    pat: RcDoc<'a, ()>,
    typ: Option<RcDoc<'a, ()>>,
    expr: RcDoc<'a, ()>,
    toplevel: bool,
) -> RcDoc<'a, ()> {
    RcDoc::as_string("let")
        .append(RcDoc::space())
        .append(
            pat.append(match typ {
                None => RcDoc::nil(),
                Some(tau) => RcDoc::space()
                    .append(RcDoc::as_string(":"))
                    .append(RcDoc::space())
                    .append(tau),
            })
            .group(),
        )
        .append(RcDoc::space())
        .append(RcDoc::as_string("="))
        .group()
        .append(RcDoc::line().append(expr.group()))
        .nest(2)
        .append(if toplevel {
            RcDoc::nil()
        } else {
            RcDoc::line().append(RcDoc::as_string("in"))
        })
}

fn make_tuple<'a, I: IntoIterator<Item = RcDoc<'a, ()>>>(args: I) -> RcDoc<'a, ()> {
    RcDoc::as_string("(")
        .append(
            RcDoc::line_()
                .append(RcDoc::intersperse(
                    args.into_iter(),
                    RcDoc::as_string(",").append(RcDoc::line()),
                ))
                .group()
                .nest(2),
        )
        .append(RcDoc::line_())
        .append(RcDoc::as_string(")"))
        .group()
}

fn make_list<'a, I: IntoIterator<Item = RcDoc<'a, ()>>>(args: I) -> RcDoc<'a, ()> {
    RcDoc::as_string("[")
        .append(
            RcDoc::line_()
                .append(RcDoc::intersperse(
                    args.into_iter(),
                    RcDoc::as_string(";").append(RcDoc::line()),
                ))
                .group()
                .nest(2),
        )
        .append(RcDoc::line_())
        .append(RcDoc::as_string("]"))
        .group()
}

fn make_typ_tuple<'a, I: IntoIterator<Item = RcDoc<'a, ()>>>(args: I) -> RcDoc<'a, ()> {
    RcDoc::as_string("(")
        .append(
            RcDoc::line_()
                .append(RcDoc::intersperse(
                    args.into_iter(),
                    RcDoc::space()
                        .append(RcDoc::as_string("&"))
                        .append(RcDoc::line()),
                ))
                .group()
                .nest(2),
        )
        .append(RcDoc::line_())
        .append(RcDoc::as_string(")"))
        .group()
}

fn make_paren<'a>(e: RcDoc<'a, ()>) -> RcDoc<'a, ()> {
    RcDoc::as_string("(")
        .append(RcDoc::line_().append(e).group().nest(2))
        .append(RcDoc::as_string(")"))
        .group()
}

fn make_begin_paren<'a>(e: RcDoc<'a, ()>) -> RcDoc<'a, ()> {
    RcDoc::as_string("begin")
        .append(RcDoc::line().append(e).group().nest(2))
        .append(RcDoc::line())
        .append(RcDoc::as_string("end"))
}

fn translate_ident<'a>(x: Ident) -> RcDoc<'a, ()> {
    let ident_str = match x {
        Ident::Original(s) => s.clone(),
        Ident::Hacspec(id, s) => format!("{}_{}", s, id.0),
    };
    translate_ident_str(ident_str)
}

fn translate_ident_str<'a>(ident_str: String) -> RcDoc<'a, ()> {
    let mut ident_str = ident_str.clone();
    let secret_int_regex = Regex::new(r"(?P<prefix>(U|I))(?P<digits>\d{1,3})").unwrap();
    ident_str = secret_int_regex
        .replace_all(&ident_str, r"${prefix}int${digits}")
        .to_string();
    let secret_signed_int_fix = Regex::new(r"iint").unwrap();
    ident_str = secret_signed_int_fix
        .replace_all(&ident_str, "int")
        .to_string();
    let mut snake_case_ident = ident_str.to_snake_case();
    if snake_case_ident == "new" {
        snake_case_ident = "new_".to_string();
    }
    RcDoc::as_string(snake_case_ident)
}

fn translate_base_typ<'a>(tau: BaseTyp) -> RcDoc<'a, ()> {
    match tau {
        BaseTyp::Unit => RcDoc::as_string("unit"),
        BaseTyp::Bool => RcDoc::as_string("bool"),
        BaseTyp::UInt8 => RcDoc::as_string("pub_uint8"),
        BaseTyp::Int8 => RcDoc::as_string("pub_int8"),
        BaseTyp::UInt16 => RcDoc::as_string("pub_uin16"),
        BaseTyp::Int16 => RcDoc::as_string("pub_int16"),
        BaseTyp::UInt32 => RcDoc::as_string("pub_uint32"),
        BaseTyp::Int32 => RcDoc::as_string("pub_int32"),
        BaseTyp::UInt64 => RcDoc::as_string("pub_uint64"),
        BaseTyp::Int64 => RcDoc::as_string("pub_int64"),
        BaseTyp::UInt128 => RcDoc::as_string("pub_uint128"),
        BaseTyp::Int128 => RcDoc::as_string("pub_int128"),
        BaseTyp::Usize => RcDoc::as_string("uint_size"),
        BaseTyp::Isize => RcDoc::as_string("int_size"),
        BaseTyp::Str => RcDoc::as_string("string"),
        BaseTyp::Seq(tau) => {
            let tau: BaseTyp = tau.0;
            RcDoc::as_string("seq")
                .append(RcDoc::space())
                .append(translate_base_typ(tau))
                .group()
        }
        BaseTyp::Array(size, tau) => {
            let tau = tau.0;
            RcDoc::as_string("lseq")
                .append(RcDoc::space())
                .append(translate_base_typ(tau))
                .append(RcDoc::space())
                .append(RcDoc::as_string(match &size.0 {
                    ArraySize::Ident(id) => format!("{}", id),
                    ArraySize::Integer(i) => format!("{}", i),
                }))
                .group()
        }
        BaseTyp::Named(ident, args) => translate_ident(ident.0).append(match args {
            None => RcDoc::nil(),
            Some(args) => RcDoc::space().append(RcDoc::intersperse(
                args.iter().map(|arg| translate_base_typ(arg.0.clone())),
                RcDoc::space(),
            )),
        }),
        BaseTyp::Variable(id) => RcDoc::as_string(format!("'t{}", id.0)),
        BaseTyp::Tuple(args) => {
            make_typ_tuple(args.into_iter().map(|(arg, _)| translate_base_typ(arg)))
        }
        BaseTyp::NaturalInteger(_secrecy, modulo) => RcDoc::as_string("nat_mod")
            .append(RcDoc::space())
            .append(RcDoc::as_string(format!("0x{}", &modulo.0))),
    }
}

fn translate_typ((_, (tau, _)): &Typ) -> RcDoc<()> {
    translate_base_typ(tau.clone())
}

fn translate_literal(lit: &Literal) -> RcDoc<()> {
    match lit {
        Literal::Unit => RcDoc::as_string("()"),
        Literal::Bool(true) => RcDoc::as_string("true"),
        Literal::Bool(false) => RcDoc::as_string("false"),
        Literal::Int128(x) => RcDoc::as_string(format!("pub_i128 {:#x}", x)),
        Literal::UInt128(x) => RcDoc::as_string(format!("pub_u128 {:#x}", x)),
        Literal::Int64(x) => RcDoc::as_string(format!("pub_i64 {:#x}", x)),
        Literal::UInt64(x) => RcDoc::as_string(format!("pub_u64 {:#x}", x)),
        Literal::Int32(x) => RcDoc::as_string(format!("pub_i32 {:#x}", x)),
        Literal::UInt32(x) => RcDoc::as_string(format!("pub_u32 {:#x}", x)),
        Literal::Int16(x) => RcDoc::as_string(format!("pub_i16 {:#x}", x)),
        Literal::UInt16(x) => RcDoc::as_string(format!("pub_u16 {:#x}", x)),
        Literal::Int8(x) => RcDoc::as_string(format!("pub_i8 {:#x}", x)),
        Literal::UInt8(x) => RcDoc::as_string(format!("pub_u8 {:#x}", x)),
        Literal::Isize(x) => RcDoc::as_string(format!("isize {}", x)),
        Literal::Usize(x) => RcDoc::as_string(format!("usize {}", x)),
        Literal::Str(msg) => RcDoc::as_string(format!("\"{}\"", msg)),
    }
}

fn translate_pattern(p: &Pattern) -> RcDoc<()> {
    match p {
        Pattern::IdentPat(x) => translate_ident(x.clone()),
        Pattern::WildCard => RcDoc::as_string("_"),
        Pattern::Tuple(pats) => make_tuple(pats.iter().map(|(pat, _)| translate_pattern(pat))),
    }
}

fn translate_binop<'a, 'b>(
    op: &'a BinOpKind,
    op_typ: &'b Typ,
    typ_dict: &TypeDict,
) -> RcDoc<'a, ()> {
    match (op, &(op_typ.1).0) {
        (_, BaseTyp::Named(ident, _)) => {
            let ident = match &ident.0 {
                Ident::Original(i) => i,
                Ident::Hacspec(_, _) => panic!(), // should not happen
            };
            match typ_dict.get(ident) {
                Some((inner_ty, entry)) => match entry {
                    DictEntry::NaturalInteger => match op {
                        BinOpKind::Sub => return RcDoc::as_string("-"),
                        BinOpKind::Add => return RcDoc::as_string("+"),
                        BinOpKind::Mul => return RcDoc::as_string("*"),
                        BinOpKind::Div => return RcDoc::as_string("/"),
                        BinOpKind::Rem => return RcDoc::as_string("%"),
                        _ => unimplemented!(),
                    },
                    DictEntry::Array | DictEntry::Alias => {
                        return translate_binop(op, inner_ty, typ_dict)
                    }
                },
                _ => (), // should not happen
            }
        }
        _ => (),
    };
    match (op, &(op_typ.1).0) {
        (BinOpKind::Sub, BaseTyp::Usize) | (BinOpKind::Sub, BaseTyp::Isize) => {
            RcDoc::as_string("-")
        }
        (BinOpKind::Add, BaseTyp::Usize) | (BinOpKind::Add, BaseTyp::Isize) => {
            RcDoc::as_string("+")
        }
        (BinOpKind::Mul, BaseTyp::Usize) | (BinOpKind::Mul, BaseTyp::Isize) => {
            RcDoc::as_string("*")
        }
        (BinOpKind::Div, BaseTyp::Usize) | (BinOpKind::Div, BaseTyp::Isize) => {
            RcDoc::as_string("/")
        }
        (BinOpKind::Sub, BaseTyp::Seq(_)) | (BinOpKind::Sub, BaseTyp::Array(_, _)) => {
            RcDoc::as_string("`seq_minus`")
        }
        (BinOpKind::Add, BaseTyp::Seq(_)) | (BinOpKind::Add, BaseTyp::Array(_, _)) => {
            RcDoc::as_string("`seq_add`")
        }
        (BinOpKind::Mul, BaseTyp::Seq(_)) | (BinOpKind::Mul, BaseTyp::Array(_, _)) => {
            RcDoc::as_string("`seq_mul`")
        }
        (BinOpKind::Div, BaseTyp::Seq(_)) | (BinOpKind::Div, BaseTyp::Array(_, _)) => {
            RcDoc::as_string("`seq_div`")
        }
        (BinOpKind::BitXor, BaseTyp::Seq(_)) | (BinOpKind::BitXor, BaseTyp::Array(_, _)) => {
            RcDoc::as_string("`seq_xor`")
        }
        (BinOpKind::BitAnd, BaseTyp::Seq(_)) | (BinOpKind::BitAnd, BaseTyp::Array(_, _)) => {
            RcDoc::as_string("`seq_and`")
        }
        (BinOpKind::BitOr, BaseTyp::Seq(_)) | (BinOpKind::BitOr, BaseTyp::Array(_, _)) => {
            RcDoc::as_string("`seq_or`")
        }
        (BinOpKind::Sub, _) => RcDoc::as_string("-."),
        (BinOpKind::Add, _) => RcDoc::as_string("+."),
        (BinOpKind::Mul, _) => RcDoc::as_string("*."),
        (BinOpKind::Div, _) => RcDoc::as_string("/."),
        (BinOpKind::Rem, _) => RcDoc::as_string("%."),
        (BinOpKind::BitXor, _) => RcDoc::as_string("^."),
        (BinOpKind::BitAnd, _) => RcDoc::as_string("&."),
        (BinOpKind::BitOr, _) => RcDoc::as_string("|."),
        (BinOpKind::Shl, _) => RcDoc::as_string("`shift_left`"),
        (BinOpKind::Shr, _) => RcDoc::as_string("`shift_right`"),
        (BinOpKind::Lt, _) => RcDoc::as_string("<."),
        (BinOpKind::Le, _) => RcDoc::as_string("<=."),
        (BinOpKind::Ge, _) => RcDoc::as_string(">=."),
        (BinOpKind::Gt, _) => RcDoc::as_string(">."),
        (BinOpKind::Ne, _) => RcDoc::as_string("!="),
        (BinOpKind::Eq, _) => RcDoc::as_string("=="),
        (BinOpKind::And, _) => RcDoc::as_string("&&"),
        (BinOpKind::Or, _) => RcDoc::as_string("||"),
    }
}

fn translate_unop<'a, 'b>(op: &'a UnOpKind, _op_typ: &'b Typ) -> RcDoc<'a, ()> {
    match op {
        UnOpKind::Not => RcDoc::as_string("~"),
        UnOpKind::Neg => RcDoc::as_string("-"),
    }
}

#[derive(Debug)]
enum FuncPrefix {
    Regular,
    Array(ArraySize),
    NatMod(String),
}

fn translate_prefix_for_func_name<'a>(
    prefix: BaseTyp,
    typ_dict: &'a TypeDict,
) -> (RcDoc<'a, ()>, FuncPrefix) {
    match prefix {
        BaseTyp::Bool => panic!(), // should not happen
        BaseTyp::Unit => panic!(), // should not happen
        BaseTyp::UInt8 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Int8 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::UInt16 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Int16 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::UInt32 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Int32 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::UInt64 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Int64 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::UInt128 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Int128 => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Usize => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Isize => (RcDoc::as_string("int"), FuncPrefix::Regular),
        BaseTyp::Str => (RcDoc::as_string("string"), FuncPrefix::Regular),
        BaseTyp::Seq(_) => (RcDoc::as_string(SEQ_MODULE), FuncPrefix::Regular),
        BaseTyp::Array(size, _) => (
            RcDoc::as_string(SEQ_MODULE),
            FuncPrefix::Array(size.0.clone()),
        ),
        BaseTyp::Named(ident, _) => {
            // if the type is an array, we should print the Seq module instead
            match &ident.0 {
                Ident::Original(name) => match typ_dict.get(name) {
                    Some((alias_typ, DictEntry::Array))
                    | Some((alias_typ, DictEntry::Alias))
                    | Some((alias_typ, DictEntry::NaturalInteger)) => {
                        translate_prefix_for_func_name((alias_typ.1).0.clone(), typ_dict)
                    }
                    _ => (translate_ident_str(name.clone()), FuncPrefix::Regular),
                },
                Ident::Hacspec(_, _) => panic!(), // should not happen
            }
        }
        BaseTyp::Variable(_) => panic!(), // shoult not happen
        BaseTyp::Tuple(_) => panic!(),    // should not happen
        BaseTyp::NaturalInteger(_, modulo) => (
            RcDoc::as_string("nat"),
            FuncPrefix::NatMod(modulo.0.clone()),
        ),
    }
}

fn translate_func_name<'a>(
    prefix: Option<Spanned<BaseTyp>>,
    name: &'a Ident,
    typ_dict: &'a TypeDict,
) -> RcDoc<'a, ()> {
    match prefix.clone() {
        None => {
            let name = translate_ident(name.clone());
            match format!("{}", name.pretty(0)).as_str() {
                "uint128" | "uint64" | "uint32" | "uint16" | "uint8" | "int128" | "int64"
                | "int32" | "int16" | "int8" => {
                    // In this case, we're trying to apply a secret
                    // int constructor. The value it is applied to is
                    // a public integer of the same kind. So in F*, that
                    // will amount to a classification operation
                    RcDoc::as_string("secret")
                }
                _ => name,
            }
        }
        Some((prefix, _)) => {
            let (module_name, prefix_info) =
                translate_prefix_for_func_name(prefix.clone(), typ_dict);
            let type_arg = match prefix.clone() {
                BaseTyp::Seq(tau) => Some(translate_base_typ(tau.0.clone())),
                BaseTyp::Array(_, tau) => Some(translate_base_typ(tau.0.clone())),
                _ => None,
            };
            let func_ident = translate_ident(name.clone());
            module_name
                .clone()
                .append(RcDoc::as_string("_"))
                .append(func_ident.clone())
                .append(
                    match (
                        format!("{}", module_name.pretty(0)).as_str(),
                        format!("{}", func_ident.pretty(0)).as_str(),
                    ) {
                        ("seq", "new_") | ("seq", "from_slice") | ("seq", "from_slice_range") => {
                            match prefix_info {
                                FuncPrefix::Array(ArraySize::Ident(s)) => {
                                    RcDoc::space().append(translate_ident_str(s))
                                }
                                FuncPrefix::Array(ArraySize::Integer(i)) => {
                                    RcDoc::space().append(RcDoc::as_string(format!("{}", i)))
                                }
                                FuncPrefix::Regular => {
                                    // This is the Seq case, should be alright
                                    RcDoc::nil()
                                }
                                _ => panic!(), // should not happen
                            }
                        }
                        _ => RcDoc::nil(),
                    },
                )
                .append(match type_arg {
                    None => RcDoc::nil(),
                    Some(arg) => RcDoc::space().append(RcDoc::as_string("#")).append(arg),
                })
        }
    }
}

fn translate_expression<'a>(e: &'a Expression, typ_dict: &'a TypeDict) -> RcDoc<'a, ()> {
    match e {
        Expression::Binary((op, _), ref e1, ref e2, op_typ) => {
            let e1 = &e1.0;
            let e2 = &e2.0;
            make_paren(translate_expression(e1, typ_dict))
                .append(RcDoc::space())
                .append(translate_binop(op, op_typ.as_ref().unwrap(), typ_dict))
                .append(RcDoc::space())
                .append(make_paren(translate_expression(e2, typ_dict)))
                .group()
        }
        Expression::Unary(op, e1, op_typ) => {
            let e1 = &e1.0;
            translate_unop(op, op_typ.as_ref().unwrap())
                .append(RcDoc::space())
                .append(make_paren(translate_expression(e1, typ_dict)))
                .group()
        }
        Expression::Lit(lit) => translate_literal(lit),
        Expression::Tuple(es) => make_tuple(
            es.into_iter()
                .map(|(e, _)| translate_expression(&e, typ_dict)),
        ),
        Expression::Named(p) => translate_ident(p.clone()),
        Expression::FuncCall(prefix, name, args) => {
            translate_func_name(prefix.clone(), &name.0, typ_dict).append(RcDoc::concat(
                args.iter().map(|((arg, _), _)| {
                    RcDoc::space().append(make_paren(translate_expression(arg, typ_dict)))
                }),
            ))
        }
        Expression::MethodCall(sel_arg, sel_typ, (f, _), args) => {
            translate_func_name(sel_typ.clone().map(|x| x.1), f, typ_dict)
                .append(
                    RcDoc::space()
                        .append(make_paren(translate_expression(&(sel_arg.0).0, typ_dict))),
                )
                .append(RcDoc::concat(args.iter().map(|((arg, _), _)| {
                    RcDoc::space().append(make_paren(translate_expression(arg, typ_dict)))
                })))
        }
        Expression::ArrayIndex(x, e2) => {
            let e2 = &e2.0;
            RcDoc::as_string("array_index")
                .append(RcDoc::space())
                .append(make_paren(translate_ident(x.0.clone())))
                .append(RcDoc::space())
                .append(make_paren(translate_expression(e2, typ_dict)))
        }
        Expression::NewArray(_, _, args) => RcDoc::as_string(format!("{}_from_list", SEQ_MODULE))
            .append(RcDoc::space())
            .append(make_list(
                args.iter().map(|(e, _)| translate_expression(e, typ_dict)),
            )),
        Expression::IntegerCasting(_, _) => unimplemented!(),
    }
}

fn translate_statement<'a>(s: &'a Statement, typ_dict: &'a TypeDict) -> RcDoc<'a, ()> {
    match s {
        Statement::LetBinding((pat, _), typ, (expr, _)) => make_let_binding(
            translate_pattern(pat),
            typ.as_ref().map(|(typ, _)| translate_typ(typ)),
            translate_expression(expr, typ_dict),
            false,
        ),
        Statement::Reassignment((x, _), (e1, _)) => make_let_binding(
            translate_ident(x.clone()),
            None,
            translate_expression(e1, typ_dict),
            false,
        ),
        Statement::ArrayUpdate((x, _), (e1, _), (e2, _)) => make_let_binding(
            translate_ident(x.clone()),
            None,
            RcDoc::as_string("array_upd")
                .append(RcDoc::space())
                .append(translate_ident(x.clone()))
                .append(RcDoc::space())
                .append(make_paren(translate_expression(e1, typ_dict)))
                .append(RcDoc::space())
                .append(make_paren(translate_expression(e2, typ_dict))),
            false,
        ),
        Statement::ReturnExp(e1) => translate_expression(e1, typ_dict),
        Statement::Conditional((cond, _), (b1, _), b2, mutated) => {
            let mutated_info = mutated.as_ref().unwrap().as_ref();
            make_let_binding(
                make_tuple(mutated_info.vars.iter().map(|i| translate_ident(i.clone()))),
                None,
                RcDoc::as_string("if")
                    .append(RcDoc::space())
                    .append(translate_expression(cond, typ_dict))
                    .append(RcDoc::space())
                    .append(RcDoc::as_string("then"))
                    .append(RcDoc::space())
                    .append(make_begin_paren(
                        translate_block(b1, true, typ_dict)
                            .append(RcDoc::hardline())
                            .append(translate_statement(&mutated_info.stmt, typ_dict)),
                    ))
                    .append(match b2 {
                        None => RcDoc::space()
                            .append(RcDoc::as_string("else"))
                            .append(RcDoc::space())
                            .append(make_begin_paren(translate_statement(
                                &mutated_info.stmt,
                                typ_dict,
                            ))),
                        Some((b2, _)) => RcDoc::space()
                            .append(RcDoc::as_string("else"))
                            .append(RcDoc::space())
                            .append(make_begin_paren(
                                translate_block(b2, true, typ_dict)
                                    .append(RcDoc::hardline())
                                    .append(translate_statement(&mutated_info.stmt, typ_dict)),
                            )),
                    }),
                false,
            )
        }
        Statement::ForLoop((x, _), (e1, _), (e2, _), (b, _)) => {
            let mutated_info = b.mutated.as_ref().unwrap().as_ref();
            let mut_tuple =
                make_tuple(mutated_info.vars.iter().map(|i| translate_ident(i.clone())));
            let closure_tuple = make_tuple(vec![translate_ident(x.clone()), mut_tuple.clone()]);
            let loop_expr = RcDoc::as_string("foldi")
                .append(RcDoc::space())
                .append(make_paren(translate_expression(e1, typ_dict)))
                .append(RcDoc::space())
                .append(make_paren(translate_expression(e2, typ_dict)))
                .append(RcDoc::space())
                .append(RcDoc::as_string("(fun"))
                .append(RcDoc::space())
                .append(closure_tuple)
                .append(RcDoc::space())
                .append(RcDoc::as_string("->"))
                .append(RcDoc::line())
                .append(translate_block(b, true, typ_dict))
                .append(RcDoc::hardline())
                .append(translate_statement(&mutated_info.stmt, typ_dict))
                .append(RcDoc::as_string(")"))
                .group()
                .nest(2)
                .append(RcDoc::line())
                .append(mut_tuple.clone());
            make_let_binding(mut_tuple, None, loop_expr, false)
        }
    }
    .group()
}

fn translate_block<'a>(
    b: &'a Block,
    omit_extra_unit: bool,
    typ_dict: &'a TypeDict,
) -> RcDoc<'a, ()> {
    RcDoc::intersperse(
        b.stmts
            .iter()
            .map(|(i, _)| translate_statement(i, typ_dict).group()),
        RcDoc::hardline(),
    )
    .append(match (&b.return_typ, omit_extra_unit) {
        (None, _) => panic!(), // should not happen,
        (Some(((Borrowing::Consumed, _), (BaseTyp::Unit, _))), false) => {
            RcDoc::hardline().append(RcDoc::as_string("()"))
        }
        (Some(_), _) => RcDoc::nil(),
    })
}

fn translate_item<'a>(i: &'a Item, typ_dict: &'a TypeDict) -> RcDoc<'a, ()> {
    match i {
        Item::FnDecl((f, _), sig, (b, _)) => make_let_binding(
            translate_ident(f.clone())
                .append(RcDoc::line())
                .append(if sig.args.len() > 0 {
                    RcDoc::intersperse(
                        sig.args.iter().map(|((x, _), (tau, _))| {
                            make_paren(
                                translate_ident(x.clone())
                                    .append(RcDoc::space())
                                    .append(RcDoc::as_string(":"))
                                    .append(RcDoc::space())
                                    .append(translate_typ(tau)),
                            )
                        }),
                        RcDoc::line(),
                    )
                } else {
                    RcDoc::as_string("()")
                })
                .append(RcDoc::line())
                .append(
                    RcDoc::as_string(":")
                        .append(RcDoc::space())
                        .append(translate_base_typ(sig.ret.0.clone()))
                        .group(),
                ),
            None,
            translate_block(b, false, typ_dict)
                .append(if let BaseTyp::Unit = sig.ret.0 {
                    RcDoc::hardline().append(RcDoc::as_string("()"))
                } else {
                    RcDoc::nil()
                })
                .group(),
            true,
        ),
        Item::ArrayDecl(name, size, cell_t) => RcDoc::as_string("type")
            .append(RcDoc::space())
            .append(translate_ident(name.0.clone()))
            .append(RcDoc::space())
            .append(RcDoc::as_string("="))
            .group()
            .append(
                RcDoc::line()
                    .append(RcDoc::as_string("lseq"))
                    .append(RcDoc::space())
                    .append(make_paren(translate_base_typ(cell_t.0.clone())))
                    .append(RcDoc::space())
                    .append(make_paren(translate_expression(&size.0, typ_dict)))
                    .group()
                    .nest(2),
            ),
        Item::ConstDecl(name, ty, e) => make_let_binding(
            translate_ident(name.0.clone()),
            Some(translate_base_typ(ty.0.clone())),
            translate_expression(&e.0, typ_dict),
            true,
        ),
        Item::NaturalIntegerDecl(nat_name, canvas_name, _secrecy, canvas_size, modulo) => {
            RcDoc::as_string("type")
                .append(RcDoc::space())
                .append(translate_ident(canvas_name.0.clone()))
                .append(RcDoc::space())
                .append(RcDoc::as_string("="))
                .group()
                .append(
                    RcDoc::line()
                        .append(RcDoc::as_string("lseq"))
                        .append(RcDoc::space())
                        .append(make_paren(translate_base_typ(BaseTyp::UInt8)))
                        .append(RcDoc::space())
                        .append(make_paren(translate_expression(&canvas_size.0, typ_dict)))
                        .group()
                        .nest(2),
                )
                .append(RcDoc::hardline())
                .append(RcDoc::hardline()) //TODO: add other decl
                .append(
                    RcDoc::as_string("type")
                        .append(RcDoc::space())
                        .append(translate_ident(nat_name.0.clone()))
                        .append(RcDoc::space())
                        .append(RcDoc::as_string("="))
                        .group()
                        .append(
                            RcDoc::line()
                                .append(RcDoc::as_string("nat_mod"))
                                .append(RcDoc::space())
                                .append(RcDoc::as_string(format!("0x{}", &modulo.0)))
                                .group()
                                .nest(2),
                        ),
                )
        }
    }
}

fn translate_program<'a>(p: &'a Program, typ_dict: &'a TypeDict) -> RcDoc<'a, ()> {
    RcDoc::concat(p.items.iter().map(|(i, _)| {
        translate_item(i, typ_dict)
            .append(RcDoc::hardline())
            .append(RcDoc::hardline())
    }))
}

pub fn translate_and_write_to_file(sess: &Session, p: &Program, file: &str, typ_dict: &TypeDict) {
    let file = file.trim();
    let path = path::Path::new(file);
    let mut file = match File::create(&path) {
        Err(why) => {
            sess.err(format!("Unable to write to outuput file {}: \"{}\"", file, why).as_str());
            return;
        }
        Ok(file) => file,
    };
    let width = 80;
    let mut w = Vec::new();
    let module_name = path.file_stem().unwrap().to_str().unwrap();
    write!(
        file,
        "module {}\n\n\
        #set-options \"--fuel 0 --ifuel 1 --z3rlimit 15\"\n\n\
        open Hacspec.Lib\n\
        open FStar.Mul\n\n",
        module_name
    )
    .unwrap();
    translate_program(p, typ_dict)
        .render(width, &mut w)
        .unwrap();
    write!(file, "{}", String::from_utf8(w).unwrap()).unwrap()
}
