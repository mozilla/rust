use super::*;
use syntax_pos::Span;
use rustc::ty::maps::QueryMsg;
use std::fs::File;
use std::time::{Duration, Instant};
use std::collections::hash_map::HashMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Query {
    pub span: Span,
    pub msg: QueryMsg,
}
pub enum Effect {
    QueryBegin(Query, CacheCase),
}
pub enum CacheCase {
    Hit, Miss
}
/// Recursive trace structure
pub struct Rec {
    pub effect: Effect,
    pub start: Instant,
    pub duration: Duration,
    pub extent: Box<Vec<Rec>>,
}
/// State for parsing recursive trace structure
#[derive(Clone, Eq, PartialEq)]
pub enum ParseState {
    NoQuery,
    HaveQuery(Query, Instant),
}
pub struct StackFrame {
    pub parse_st: ParseState,
    pub traces:   Vec<Rec>,
}
pub struct QueryMetric {
    pub count: usize,
    pub duration: Duration,
}

pub fn cons_of_query_msg(q: &trace::Query) -> String {
    let s = format!("{:?}", q.msg);
    let cons: Vec<&str> = s.split(|d| d == '(' || d == '{').collect();
    assert!(cons.len() > 0 && cons[0] != "");
    cons[0].to_string()
}

// First return value is text; second return value is a CSS class
pub fn html_of_effect(eff: &Effect) -> (String, String) {
    match *eff {
        Effect::QueryBegin(ref qmsg, ref cc) => {
            let cons = cons_of_query_msg(qmsg);
            (cons.clone(),
             format!("{} {}",
                     cons,
                     match *cc {
                         CacheCase::Hit => "hit",
                         CacheCase::Miss => "miss",
                     }))
        }
    }
}

// First return value is text; second return value is a CSS class
fn html_of_duration(_start: &Instant, dur: &Duration) -> (String, String) {
    use rustc::util::common::duration_to_secs_str;
    (duration_to_secs_str(dur.clone()),
     "".to_string()
    )
}

fn html_of_fraction(frac: f64) -> (String, String) {
    let css = {
        if       frac > 0.50  { format!("frac-50") }
        else if  frac > 0.40  { format!("frac-40") }
        else if  frac > 0.30  { format!("frac-30") }
        else if  frac > 0.20  { format!("frac-20") }
        else if  frac > 0.10  { format!("frac-10") }
        else if  frac > 0.05  { format!("frac-05") }
        else if  frac > 0.02  { format!("frac-02") }
        else if  frac > 0.01  { format!("frac-01") }
        else if  frac > 0.001 { format!("frac-001") }
        else                  { format!("frac-0") }
    };
    let percent = frac * 100 as f64;
    if percent > 0.1 as f64 { (format!("{:.1}%", percent), css) }
    else { (format!("< 0.1%", ), css) }
}

fn total_duration(traces: &Vec<Rec>) -> Duration {
    let mut sum : Duration = Duration::new(0,0);
    for t in traces.iter() {
        sum += t.duration;
    }
    return sum
}

fn duration_div(nom: Duration, den: Duration) -> f64 {
    let nom_sec = nom.as_secs();
    let den_sec = den.as_secs();
    let nom_nanos = nom.subsec_nanos();
    let den_nanos = den.subsec_nanos();
    if nom_sec == den_sec {
        if nom_sec == 0 {
            nom_nanos as f64 / den_nanos as f64
        } else {
            panic!("FIXME(matthewhammer)")
        }
    } else {
        panic!("FIXME(matthewhammer)")
    }
}

fn write_traces_rec(file: &mut File, traces: &Vec<Rec>, total: Duration, depth: usize) {
    for t in traces {
        let (eff_text, eff_css_classes) = html_of_effect(&t.effect);
        let (dur_text, dur_css_classes) = html_of_duration(&t.start, &t.duration);
        let fraction = duration_div(t.duration, total);
        let percent = fraction * 100 as f64;
        let (frc_text, frc_css_classes) = html_of_fraction(fraction);
        write!(file, "<div class=\"trace depth-{} extent-{}{} {} {} {}\">\n",
               depth,
               t.extent.len(),
               /* Heuristic for 'important' CSS class: */
               if t.extent.len() > 5 || percent >= 1.0 as f64 {
                   " important" }
               else { "" },
               eff_css_classes,
               dur_css_classes,
               frc_css_classes,
        ).unwrap();
        write!(file, "<div class=\"eff\">{}</div>\n", eff_text).unwrap();
        write!(file, "<div class=\"dur\">{}</div>\n", dur_text).unwrap();
        write!(file, "<div class=\"frc\">{}</div>\n", frc_text).unwrap();
        write_traces_rec(file, &t.extent, total, depth + 1);
        write!(file, "</div>\n").unwrap();
    }
}

fn compute_counts_rec(counts: &mut HashMap<String,QueryMetric>, traces: &Vec<Rec>) {
    for t in traces.iter() {
        match t.effect {
            Effect::QueryBegin(ref qmsg, ref _cc) => {
                let qcons = cons_of_query_msg(qmsg);
                let qm = match counts.get(&qcons) {
                    Some(qm) =>
                        QueryMetric{
                            count: qm.count + 1,
                            duration: qm.duration + t.duration
                        },
                    None => QueryMetric{
                        count: 1,
                        duration: t.duration
                    }
                };
                counts.insert(qcons, qm);
            }
        }
        compute_counts_rec(counts, &t.extent)
    }
}

pub fn write_counts(count_file: &mut File, counts: &mut HashMap<String,QueryMetric>) {
    use rustc::util::common::duration_to_secs_str;
    use std::cmp::Ordering;

    let mut data = vec![];
    for (ref cons, ref qm) in counts.iter() {
        data.push((cons.clone(), qm.count.clone(), qm.duration.clone()));
    };
    data.sort_by(|&(_,_,d1),&(_,_,d2)|
                 if d1 > d2 { Ordering::Less } else { Ordering::Greater } );
    for (cons, count, duration) in data {
        write!(count_file, "{},{},{}\n",
               cons, count, duration_to_secs_str(duration)
        ).unwrap();
    }
}

pub fn write_traces(html_file: &mut File, counts_file: &mut File, traces: &Vec<Rec>) {
    let mut counts : HashMap<String,QueryMetric> = HashMap::new();
    compute_counts_rec(&mut counts, traces);
    write_counts(counts_file, &mut counts);

    let total : Duration = total_duration(traces);
    write_traces_rec(html_file, traces, total, 0)
}

pub fn write_style(html_file: &mut File) {
    write!(html_file,"{}", "
body {
    font-family: sans-serif;
    background: black;
}
.trace {
    color: black;
    display: inline-block;
    border-style: solid;
    border-color: red;
    border-width: 1px;
    border-radius: 5px;
    padding: 0px;
    margin: 1px;
    font-size: 0px;
}
.miss {
    border-color: red;
    border-width: 1px;
}
.extent-0 {
    padding: 2px;
}
.important {
    border-width: 3px;
    font-size: 12px;
    color: white;
    border-color: #f77;
}
.hit {
    padding: 0px;
    border-color: blue;
    border-width: 3px;
}
.eff {
  color: #fff;
  display: inline-block;
}
.frc {
  color: #7f7;
  display: inline-block;
}
.dur {
  display: none
}
").unwrap();
}
