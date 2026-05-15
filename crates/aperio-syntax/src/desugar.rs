//! Topic-reference desugaring.
//!
//! After parsing, the AST carries `BusSubject::Topic(name)` and
//! type-less `Subscribe { ty: None } / Publish { ty: None }` for
//! topic-ref forms (`subscribe Foo as h;`), plus
//! `Stmt::Send { subject: Expr::Ident("Foo"), ... }` for
//! topic-ref sends (`Foo <- expr`). Downstream stages (codegen,
//! interpreter) work against legacy literal-subject forms; this
//! pass normalizes the AST so they don't have to know topics
//! exist.
//!
//! Transformations:
//!   - `BusSubject::Topic(i)` → `BusSubject::Literal { subject:
//!     <wire_subject>, span: i.span }` and `ty: None` filled with
//!     the topic's declared payload type expression. The
//!     wire_subject is the dot-joined parent chain of own-subject
//!     segments (root-to-leaf).
//!   - `Stmt::Send { subject: Expr::Ident("Foo"), ... }` where
//!     `Foo` names a topic → `subject: Expr::Literal(String(
//!     <wire_subject>), span)`.
//!   - `desugar_intra_locus_topics` (Phase 2 closed-world): when
//!     a topic is used only intra-locus and has no binding,
//!     rewrites the publisher's `Stmt::Send` to a direct
//!     `self.handler(payload)` method call, sidestepping bus
//!     dispatch entirely. Runs BEFORE `desugar_topics` so the
//!     remaining bus refs go through the standard literal-subject
//!     rewrite.
//!
//! Type checking runs BEFORE this pass so topic-specific
//! diagnostics (handler-sig match, etc.) still see the original
//! `BusSubject::Topic` form and can cite the topic name in
//! errors. Codegen + runtime run AFTER, and see only the
//! literal-subject form (or, for optimized intra-locus topics,
//! direct method calls instead of Send statements).

use std::collections::BTreeMap;

use crate::ast::*;

/// Per-topic data the desugar pass needs: payload type (to fill
/// `ty: None` slots) and wire_subject (the literal subject string
/// that the topic ref desugars to). Wire subject is the dot-joined
/// chain of own-subject segments root-to-leaf — for a top-level
/// topic with no `subject:` field, it equals the topic name.
#[derive(Debug, Clone)]
struct TopicEntry {
    payload: TypeExpr,
    wire_subject: String,
}

/// Walk `program` and rewrite topic references into literal
/// forms in place. Caller invokes this after typecheck and
/// before codegen / interpretation. Idempotent: re-running on
/// already-desugared input is a no-op.
pub fn desugar_topics(program: &mut Program) {
    let mut topics: BTreeMap<String, TopicEntry> = BTreeMap::new();
    collect_topics(&program.items, &mut topics);
    rewrite_items(&mut program.items, &topics);
}

fn collect_topics(items: &[TopDecl], topics: &mut BTreeMap<String, TopicEntry>) {
    // First gather raw decls (name → (payload, parent, subject))
    // by walking the program tree. Then resolve wire_subject
    // bottom-up by following parent chains.
    #[derive(Clone)]
    struct Raw {
        payload: TypeExpr,
        parent: Option<String>,
        subject: String,
    }
    let mut raw: BTreeMap<String, Raw> = BTreeMap::new();
    fn gather(items: &[TopDecl], raw: &mut BTreeMap<String, Raw>) {
        for item in items {
            match item {
                TopDecl::Topic(t) => {
                    let subject = t.subject.clone().unwrap_or_else(|| t.name.name.clone());
                    raw.insert(
                        t.name.name.clone(),
                        Raw {
                            payload: t.payload.clone(),
                            parent: t.parent.as_ref().map(|i| i.name.clone()),
                            subject,
                        },
                    );
                }
                TopDecl::Module(m) => gather(&m.items, raw),
                _ => {}
            }
        }
    }
    gather(items, &mut raw);

    // Resolve wire_subject for each. Cycles + missing parents
    // would have already been errored by the type-resolve pass;
    // here we treat them defensively (fall back to own subject).
    for (name, r) in raw.iter() {
        let mut chain: Vec<String> = vec![r.subject.clone()];
        let mut visited: Vec<String> = vec![name.clone()];
        let mut cur = r.parent.clone();
        while let Some(p) = cur {
            if visited.contains(&p) {
                // Cycle defense.
                break;
            }
            visited.push(p.clone());
            match raw.get(&p) {
                Some(pr) => {
                    chain.push(pr.subject.clone());
                    cur = pr.parent.clone();
                }
                None => break,
            }
        }
        chain.reverse();
        topics.insert(
            name.clone(),
            TopicEntry {
                payload: r.payload.clone(),
                wire_subject: chain.join("."),
            },
        );
    }
}

fn rewrite_items(items: &mut [TopDecl], topics: &BTreeMap<String, TopicEntry>) {
    for item in items {
        match item {
            TopDecl::Locus(l) => rewrite_locus(l, topics),
            TopDecl::Fn(f) => rewrite_block(&mut f.body, topics),
            TopDecl::Module(m) => rewrite_items(&mut m.items, topics),
            _ => {}
        }
    }
}

fn rewrite_locus(l: &mut LocusDecl, topics: &BTreeMap<String, TopicEntry>) {
    for member in &mut l.members {
        match member {
            LocusMember::Bus(bb) => {
                for bm in &mut bb.members {
                    rewrite_bus_member(bm, topics);
                }
            }
            LocusMember::Lifecycle(lc) => rewrite_block(&mut lc.body, topics),
            LocusMember::Mode(md) => rewrite_block(&mut md.body, topics),
            LocusMember::Fn(fd) => rewrite_block(&mut fd.body, topics),
            _ => {}
        }
    }
}

fn rewrite_bus_member(bm: &mut BusMember, topics: &BTreeMap<String, TopicEntry>) {
    match bm {
        BusMember::Subscribe { subject, ty, .. } => {
            if let BusSubject::Topic(ident) = subject {
                let name = ident.name.clone();
                let span = ident.span;
                if let Some(entry) = topics.get(&name) {
                    if ty.is_none() {
                        *ty = Some(entry.payload.clone());
                    }
                    *subject = BusSubject::Literal {
                        subject: entry.wire_subject.clone(),
                        span,
                    };
                } else {
                    // Defensive: unresolved topic-ref keeps the
                    // ident name so a downstream "unknown subject"
                    // error has something to cite.
                    *subject = BusSubject::Literal { subject: name, span };
                }
            }
        }
        BusMember::Publish { subject, ty, .. } => {
            if let BusSubject::Topic(ident) = subject {
                let name = ident.name.clone();
                let span = ident.span;
                if let Some(entry) = topics.get(&name) {
                    if ty.is_none() {
                        *ty = Some(entry.payload.clone());
                    }
                    *subject = BusSubject::Literal {
                        subject: entry.wire_subject.clone(),
                        span,
                    };
                } else {
                    *subject = BusSubject::Literal { subject: name, span };
                }
            }
        }
    }
}

fn rewrite_block(b: &mut Block, topics: &BTreeMap<String, TopicEntry>) {
    for stmt in &mut b.stmts {
        rewrite_stmt(stmt, topics);
    }
    if let Some(tail) = &mut b.tail {
        rewrite_expr(tail, topics);
    }
}

fn rewrite_stmt(s: &mut Stmt, topics: &BTreeMap<String, TopicEntry>) {
    match s {
        Stmt::Send { subject, .. } => {
            // Rewrite `Foo <- value` to `"<wire_subject>" <- value`
            // when `Foo` is a declared topic. Subject is the only
            // place a topic ident appears in expression position
            // (typechecker rejects topic idents elsewhere).
            if let Expr::Ident(id) = subject {
                if let Some(entry) = topics.get(&id.name) {
                    let span = id.span;
                    *subject = Expr::Literal(
                        Literal::String(entry.wire_subject.clone()),
                        span,
                    );
                }
            }
        }
        Stmt::If(if_stmt) => rewrite_if(if_stmt, topics),
        Stmt::Match(m) => rewrite_match(m, topics),
        Stmt::For { body, .. } => rewrite_block(body, topics),
        Stmt::While { body, .. } => rewrite_block(body, topics),
        Stmt::Block(b) => rewrite_block(b, topics),
        Stmt::Expr(e) => rewrite_expr(e, topics),
        _ => {}
    }
}

fn rewrite_if(if_stmt: &mut IfStmt, topics: &BTreeMap<String, TopicEntry>) {
    rewrite_block(&mut if_stmt.then_block, topics);
    if let Some(else_branch) = &mut if_stmt.else_block {
        rewrite_else_branch(else_branch, topics);
    }
}

fn rewrite_else_branch(eb: &mut ElseBranch, topics: &BTreeMap<String, TopicEntry>) {
    match eb {
        ElseBranch::Else(b) => rewrite_block(b, topics),
        ElseBranch::ElseIf(if_stmt) => rewrite_if(if_stmt, topics),
    }
}

fn rewrite_match(m: &mut MatchStmt, topics: &BTreeMap<String, TopicEntry>) {
    for arm in &mut m.arms {
        match &mut arm.body {
            MatchArmBody::Block(b) => rewrite_block(b, topics),
            MatchArmBody::Expr(e) => rewrite_expr(e, topics),
        }
    }
}

fn rewrite_expr(e: &mut Expr, topics: &BTreeMap<String, TopicEntry>) {
    match e {
        Expr::Block(b) => rewrite_block(b, topics),
        Expr::If(if_stmt) => rewrite_if(if_stmt, topics),
        Expr::Match(m) => rewrite_match(m, topics),
        _ => {}
    }
}

// ---------------------------------------------------------------
// Closed-world topology optimization: intra-locus direct call.
// ---------------------------------------------------------------
//
// A topic is "intra-locus optimizable" when ALL of:
//   - no `bindings { Topic: ... }` entry references it
//   - exactly one locus type publishes it
//   - exactly one locus type subscribes it
//   - publisher locus == subscriber locus (same type)
//
// Under those conditions every Send for this topic happens
// inside an instance of the same locus that hosts the handler;
// the publish→queue→drain→dispatch path is observable as a
// straight `self.handler(payload)`. We rewrite the Send to that
// direct call at desugar time so codegen never sees the bus
// hop. The publish/subscribe entries stay in place — they
// continue to type-check and the bus runtime simply never sees
// any traffic on the optimized subject.
//
// Run BEFORE desugar_topics so the Send still has its
// `Expr::Ident(Topic)` shape (post-desugar, it'd be a literal
// string and we'd lose the cheap topic-name lookup).

/// Intra-locus optimization entry point. Mutates `program` in
/// place. Idempotent: re-running on already-optimized input is a
/// no-op (rewritten Sends become method-call Stmt::Expr nodes,
/// which the rewrite step skips).
pub fn desugar_intra_locus_topics(program: &mut Program) {
    let bindings = collect_bindings(&program.items);
    let (pubs, subs) = collect_pub_sub(&program.items);

    // Identify topic → (locus, handler) pairs eligible for the
    // direct-call rewrite.
    let mut eligible: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (topic, pub_loci) in &pubs {
        if bindings.contains(topic) {
            continue;
        }
        if pub_loci.len() != 1 {
            continue;
        }
        let pub_locus = pub_loci[0].clone();
        let sub_pairs = match subs.get(topic) {
            Some(s) => s,
            None => continue,
        };
        if sub_pairs.len() != 1 {
            continue;
        }
        let (sub_locus, handler) = &sub_pairs[0];
        if sub_locus != &pub_locus {
            continue;
        }
        eligible.insert(topic.clone(), (pub_locus, handler.clone()));
    }

    if eligible.is_empty() {
        return;
    }

    // Walk locus methods and rewrite matching Sends.
    for item in &mut program.items {
        match item {
            TopDecl::Locus(l) => intra_rewrite_locus(l, &eligible),
            TopDecl::Module(m) => intra_rewrite_module(m, &eligible),
            _ => {}
        }
    }
}

fn intra_rewrite_module(
    m: &mut ModuleDecl,
    eligible: &BTreeMap<String, (String, String)>,
) {
    for item in &mut m.items {
        match item {
            TopDecl::Locus(l) => intra_rewrite_locus(l, eligible),
            TopDecl::Module(inner) => intra_rewrite_module(inner, eligible),
            _ => {}
        }
    }
}

/// Rewrite all Send sites inside `l`'s method bodies. Only the
/// loci named as the publisher of an eligible topic get sends
/// rewritten — others may publish to other (unoptimized) topics
/// from the same lexical position, so we have to gate by the
/// (current locus name, topic name) pair.
fn intra_rewrite_locus(
    l: &mut LocusDecl,
    eligible: &BTreeMap<String, (String, String)>,
) {
    let locus_name = l.name.name.clone();
    for member in &mut l.members {
        match member {
            LocusMember::Lifecycle(lc) => {
                intra_rewrite_block(&mut lc.body, &locus_name, eligible);
            }
            LocusMember::Mode(md) => {
                intra_rewrite_block(&mut md.body, &locus_name, eligible);
            }
            LocusMember::Fn(fd) => {
                intra_rewrite_block(&mut fd.body, &locus_name, eligible);
            }
            LocusMember::Failure(f) => {
                intra_rewrite_block(&mut f.body, &locus_name, eligible);
            }
            _ => {}
        }
    }
}

fn intra_rewrite_block(
    b: &mut Block,
    locus_name: &str,
    eligible: &BTreeMap<String, (String, String)>,
) {
    for stmt in &mut b.stmts {
        intra_rewrite_stmt(stmt, locus_name, eligible);
    }
    if let Some(tail) = &mut b.tail {
        intra_rewrite_expr(tail, locus_name, eligible);
    }
}

fn intra_rewrite_stmt(
    s: &mut Stmt,
    locus_name: &str,
    eligible: &BTreeMap<String, (String, String)>,
) {
    if let Stmt::Send { subject, value, span } = s {
        if let Expr::Ident(id) = subject {
            if let Some((pub_locus, handler)) = eligible.get(&id.name) {
                if pub_locus == locus_name {
                    // Build `self.handler(value)` as a Stmt::Expr
                    // wrapping a Call(Field(KwSelf, handler), [value]).
                    let span = *span;
                    let value_expr = std::mem::replace(
                        value,
                        Expr::Literal(Literal::Bool(false), span),
                    );
                    let call_expr = Expr::Call {
                        callee: Box::new(Expr::Field {
                            receiver: Box::new(Expr::KwSelf(span)),
                            name: Ident {
                                name: handler.clone(),
                                span,
                            },
                            span,
                        }),
                        args: vec![value_expr],
                        span,
                    };
                    *s = Stmt::Expr(call_expr);
                    return;
                }
            }
        }
    }
    match s {
        Stmt::If(if_stmt) => intra_rewrite_if(if_stmt, locus_name, eligible),
        Stmt::Match(m) => intra_rewrite_match(m, locus_name, eligible),
        Stmt::For { body, .. } => intra_rewrite_block(body, locus_name, eligible),
        Stmt::While { body, .. } => intra_rewrite_block(body, locus_name, eligible),
        Stmt::Block(b) => intra_rewrite_block(b, locus_name, eligible),
        Stmt::Expr(e) => intra_rewrite_expr(e, locus_name, eligible),
        _ => {}
    }
}

fn intra_rewrite_if(
    if_stmt: &mut IfStmt,
    locus_name: &str,
    eligible: &BTreeMap<String, (String, String)>,
) {
    intra_rewrite_block(&mut if_stmt.then_block, locus_name, eligible);
    if let Some(eb) = &mut if_stmt.else_block {
        match eb.as_mut() {
            ElseBranch::Else(b) => intra_rewrite_block(b, locus_name, eligible),
            ElseBranch::ElseIf(inner) => intra_rewrite_if(inner, locus_name, eligible),
        }
    }
}

fn intra_rewrite_match(
    m: &mut MatchStmt,
    locus_name: &str,
    eligible: &BTreeMap<String, (String, String)>,
) {
    for arm in &mut m.arms {
        match &mut arm.body {
            MatchArmBody::Block(b) => intra_rewrite_block(b, locus_name, eligible),
            MatchArmBody::Expr(e) => intra_rewrite_expr(e, locus_name, eligible),
        }
    }
}

fn intra_rewrite_expr(
    e: &mut Expr,
    locus_name: &str,
    eligible: &BTreeMap<String, (String, String)>,
) {
    match e {
        Expr::Block(b) => intra_rewrite_block(b, locus_name, eligible),
        Expr::If(if_stmt) => intra_rewrite_if(if_stmt, locus_name, eligible),
        Expr::Match(m) => intra_rewrite_match(m, locus_name, eligible),
        _ => {}
    }
}

/// Topology collection helpers. Walk the program tree once,
/// gathering: which topics are bound (any binding entry), which
/// loci publish each topic, and which loci subscribe each topic
/// (with the handler ident).
fn collect_bindings(items: &[TopDecl]) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    fn walk(items: &[TopDecl], out: &mut std::collections::BTreeSet<String>) {
        for item in items {
            match item {
                TopDecl::Locus(l) => {
                    for m in &l.members {
                        if let LocusMember::Bindings(bb) = m {
                            for entry in &bb.entries {
                                out.insert(entry.topic.name.clone());
                            }
                        }
                    }
                }
                TopDecl::Module(m) => walk(&m.items, out),
                _ => {}
            }
        }
    }
    walk(items, &mut out);
    out
}

#[allow(clippy::type_complexity)]
fn collect_pub_sub(
    items: &[TopDecl],
) -> (
    BTreeMap<String, Vec<String>>,
    BTreeMap<String, Vec<(String, String)>>,
) {
    let mut pubs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut subs: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    fn walk(
        items: &[TopDecl],
        pubs: &mut BTreeMap<String, Vec<String>>,
        subs: &mut BTreeMap<String, Vec<(String, String)>>,
    ) {
        for item in items {
            match item {
                TopDecl::Locus(l) => {
                    let locus_name = l.name.name.clone();
                    for member in &l.members {
                        if let LocusMember::Bus(bb) = member {
                            for bm in &bb.members {
                                match bm {
                                    BusMember::Subscribe { subject, handler, .. } => {
                                        if let BusSubject::Topic(id) = subject {
                                            subs.entry(id.name.clone())
                                                .or_default()
                                                .push((locus_name.clone(), handler.name.clone()));
                                        }
                                    }
                                    BusMember::Publish { subject, .. } => {
                                        if let BusSubject::Topic(id) = subject {
                                            pubs.entry(id.name.clone())
                                                .or_default()
                                                .push(locus_name.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                TopDecl::Module(m) => walk(&m.items, pubs, subs),
                _ => {}
            }
        }
    }
    walk(items, &mut pubs, &mut subs);
    (pubs, subs)
}
