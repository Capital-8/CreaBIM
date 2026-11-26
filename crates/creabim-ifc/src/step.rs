//! Minimal writer for ISO 10303-21 (STEP physical file) data sections.

use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ref(pub usize);

impl std::fmt::Display for Ref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Default)]
pub struct StepWriter {
    entities: Vec<String>,
}

impl StepWriter {
    /// Add an entity, e.g. `add("IFCCARTESIANPOINT", "((0.,0.,0.))")`.
    pub fn add(&mut self, name: &str, args: impl AsRef<str>) -> Ref {
        self.entities.push(format!("{}({})", name, args.as_ref()));
        Ref(self.entities.len())
    }

    /// The objects written so far whose GlobalId is one of `guids` (as
    /// written, quoted), relationships and property sets left out. Should
    /// an opening share a guid with what fills it, the filling wins.
    pub fn objects_with(
        &self,
        guids: &std::collections::HashSet<String>,
    ) -> std::collections::HashMap<String, Ref> {
        let mut out: std::collections::HashMap<String, (Ref, bool)> =
            std::collections::HashMap::new();
        for (i, e) in self.entities.iter().enumerate() {
            let Some((class, args)) = e.split_once('(') else {
                continue;
            };
            if class.starts_with("IFCREL")
                || class.starts_with("IFCPROPERTY")
                || class == "IFCELEMENTQUANTITY"
            {
                continue;
            }
            let Some(guid) = args.split(',').next() else {
                continue;
            };
            if guids.contains(guid) {
                let opening = class == "IFCOPENINGELEMENT";
                match out.get(guid) {
                    Some((_, false)) => {}
                    Some((_, true)) if opening => {}
                    _ => {
                        out.insert(guid.to_string(), (Ref(i + 1), opening));
                    }
                }
            }
        }
        out.into_iter().map(|(k, (r, _))| (k, r)).collect()
    }

    /// The property sets given to each object so far: (the object's
    /// entity number, the set's name as written, quoted) to (the set's
    /// entity number, how many objects it is given to).
    pub fn sets_on(&self) -> std::collections::HashMap<(usize, String), (usize, usize)> {
        let mut out = std::collections::HashMap::new();
        for e in &self.entities {
            let Some(args) = e.strip_prefix("IFCRELDEFINESBYPROPERTIES(") else {
                continue;
            };
            let args = args.strip_suffix(')').unwrap_or(args);
            // GlobalId,$,$,$,RelatedObjects,RelatingPropertyDefinition
            let Some((objs, set)) = args.rsplit_once(',') else {
                continue;
            };
            let Some(set) = set.strip_prefix('#').and_then(|n| n.parse::<usize>().ok()) else {
                continue;
            };
            let Some(name) = self
                .entities
                .get(set.wrapping_sub(1))
                .and_then(|s| set_name(s))
            else {
                continue;
            };
            let objs = objs.splitn(5, ',').nth(4).unwrap_or("");
            let objs: Vec<usize> = objs
                .split(|ch: char| ch != '#' && !ch.is_ascii_digit())
                .filter_map(|t| t.strip_prefix('#')?.parse().ok())
                .collect();
            for o in &objs {
                out.insert((*o, name.clone()), (set, objs.len()));
            }
        }
        out
    }

    /// The names (quoted, as written) of the properties of set `set`.
    pub fn prop_names(&self, set: usize) -> std::collections::HashSet<String> {
        let Some(e) = self.entities.get(set.wrapping_sub(1)) else {
            return Default::default();
        };
        let list = e.rsplit_once('(').map_or("", |(_, l)| l);
        list.split(|ch: char| ch != '#' && !ch.is_ascii_digit())
            .filter_map(|t| t.strip_prefix('#')?.parse::<usize>().ok())
            .filter_map(|r| self.entities.get(r - 1))
            .filter_map(|p| first_string(p.split_once('(')?.1))
            .collect()
    }

    /// The class of entity `r`, as written ("IFCWALLTYPE").
    pub fn class_of(&self, r: Ref) -> &str {
        self.entities
            .get(r.0.wrapping_sub(1))
            .and_then(|e| e.split_once('('))
            .map_or("", |(c, _)| c)
    }

    /// Add `refs` to the list in argument `arg` of entity `r`: a type's
    /// HasPropertySets, say. `$` becomes the list.
    pub fn add_to_list_arg(&mut self, r: Ref, arg: usize, refs: &[Ref]) {
        let Some(e) = self.entities.get_mut(r.0.wrapping_sub(1)) else {
            return;
        };
        let Some((class, args)) = e.split_once('(') else {
            return;
        };
        let args = args.strip_suffix(')').unwrap_or(args);
        let mut parts = split_args(args);
        let Some(part) = parts.get_mut(arg) else {
            return;
        };
        let more = refs
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>()
            .join(",");
        *part = match part.trim() {
            "$" | "()" => format!("({more})"),
            p if p.starts_with('(') && p.ends_with(')') => format!("{},{more})", &p[..p.len() - 1]),
            _ => return,
        };
        *e = format!("{class}({})", parts.join(","));
    }

    /// Add properties `refs` to set `set`, after those it has.
    pub fn append_to_set(&mut self, set: usize, refs: &[Ref]) {
        let Some(e) = self.entities.get_mut(set.wrapping_sub(1)) else {
            return;
        };
        if refs.is_empty() || !e.ends_with("))") {
            return;
        }
        e.truncate(e.len() - 2);
        for r in refs {
            e.push_str(&format!(",{r}"));
        }
        e.push_str("))");
    }

    /// The entities referred to in argument `arg` of every `class` record
    /// (a ref or a list of refs): the objects already typed, say.
    pub fn rel_refs(&self, class: &str, arg: usize) -> std::collections::HashSet<usize> {
        let mut out = std::collections::HashSet::new();
        let head = format!("{class}(");
        for e in &self.entities {
            let Some(args) = e.strip_prefix(&head) else {
                continue;
            };
            let args = args.strip_suffix(')').unwrap_or(args);
            if let Some(a) = split_args(args).get(arg) {
                out.extend(refs_in(a));
            }
        }
        out
    }

    /// Take `gone` out of argument `arg` (a list) of every `class` record;
    /// a record left with none is dropped (numbers need not run on).
    pub fn remove_from_lists(
        &mut self,
        class: &str,
        arg: usize,
        gone: &std::collections::HashSet<usize>,
    ) {
        let head = format!("{class}(");
        for e in self.entities.iter_mut() {
            let Some(args) = e.strip_prefix(&head) else {
                continue;
            };
            let args = args.strip_suffix(')').unwrap_or(args);
            let mut parts = split_args(args);
            let Some(list) = parts.get(arg) else { continue };
            let before = refs_in(list);
            if !before.iter().any(|r| gone.contains(r)) {
                continue;
            }
            let left: Vec<String> = before
                .iter()
                .filter(|r| !gone.contains(r))
                .map(|r| format!("#{r}"))
                .collect();
            if left.is_empty() {
                e.clear();
                continue;
            }
            parts[arg] = format!("({})", left.join(","));
            *e = format!("{class}({})", parts.join(","));
        }
    }

    /// The first `class` record whose argument `arg` is `value` as written
    /// (a material by its quoted name).
    pub fn find(&self, class: &str, arg: usize, value: &str) -> Option<Ref> {
        let head = format!("{class}(");
        self.entities
            .iter()
            .position(|e| {
                e.strip_prefix(&head)
                    .map(|a| a.strip_suffix(')').unwrap_or(a))
                    .is_some_and(|a| split_args(a).get(arg).is_some_and(|x| x == value))
            })
            .map(|i| Ref(i + 1))
    }

    pub fn finish(self, header: &str) -> String {
        let mut s = String::new();
        s.push_str("ISO-10303-21;\nHEADER;\n");
        s.push_str(header);
        s.push_str("ENDSEC;\nDATA;\n");
        for (i, e) in self.entities.iter().enumerate() {
            // a record taken out leaves its number unused
            if !e.is_empty() {
                let _ = writeln!(s, "#{}={};", i + 1, e);
            }
        }
        s.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        s
    }
}

/// The entity numbers referred to in `arg` (outside quotes).
fn refs_in(arg: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut quoted = false;
    let b = arg.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            39 => quoted = !quoted,
            b'#' if !quoted => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                if let Ok(n) = arg[start..j].parse() {
                    out.push(n);
                }
                i = j;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// An entity's arguments, split at the commas outside brackets and quotes.
pub(crate) fn split_args(s: &str) -> Vec<String> {
    let (mut out, mut depth, mut quoted, mut cur) = (Vec::new(), 0i32, false, String::new());
    for ch in s.chars() {
        match ch {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    out.push(cur);
    out
}

/// The name of an `IFCPROPERTYSET(...)` record, quoted as written:
/// GlobalId,OwnerHistory,Name,... (a GlobalId has no comma).
fn set_name(e: &str) -> Option<String> {
    let mut rest = e.strip_prefix("IFCPROPERTYSET(")?;
    for _ in 0..2 {
        rest = &rest[rest.find(',')? + 1..];
    }
    first_string(rest)
}

/// The quoted string `s` starts with, quotes and all.
fn first_string(s: &str) -> Option<String> {
    let b = s.as_bytes();
    const Q: u8 = 39; // '
    if b.first() != Some(&Q) {
        return None;
    }
    let mut end = 1;
    while end < b.len() {
        if b[end] == Q {
            if b.get(end + 1) == Some(&Q) {
                end += 2;
                continue;
            }
            return Some(s[..=end].to_string());
        }
        end += 1;
    }
    None
}

/// A STEP string literal, with quotes and backslashes escaped and non-ASCII
/// text encoded as `\X2\` UTF-16 hex.
pub fn string(s: &str) -> String {
    let mut out = String::from("'");
    let mut wide: Vec<u16> = Vec::new();
    let flush = |out: &mut String, wide: &mut Vec<u16>| {
        if !wide.is_empty() {
            out.push_str("\\X2\\");
            for u in wide.drain(..) {
                let _ = write!(out, "{u:04X}");
            }
            out.push_str("\\X0\\");
        }
    };
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_control() {
            flush(&mut out, &mut wide);
            match c {
                '\'' => out.push_str("''"),
                '\\' => out.push_str("\\\\"),
                _ => out.push(c),
            }
        } else {
            // a line break or other control character goes as \X2\ too, so
            // a name written over two lines keeps them (IFC bench 2026-11-23)
            let mut buf = [0u16; 2];
            wide.extend_from_slice(c.encode_utf16(&mut buf));
        }
    }
    flush(&mut out, &mut wide);
    out.push('\'');
    out
}

/// A STEP real: always has a decimal point, exponent written as `E`.
pub fn real(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return "0.".into();
    }
    let s = format!("{v:.9}");
    s.trim_end_matches('0').to_string()
}

pub fn point3(x: f64, y: f64, z: f64) -> String {
    format!("({},{},{})", real(x), real(y), real(z))
}

pub fn point2(x: f64, y: f64) -> String {
    format!("({},{})", real(x), real(y))
}

pub fn list(items: impl IntoIterator<Item = impl std::fmt::Display>) -> String {
    let v: Vec<String> = items.into_iter().map(|i| i.to_string()).collect();
    format!("({})", v.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reals_always_have_a_point() {
        assert_eq!(real(0.0), "0.");
        assert_eq!(real(3.0), "3.");
        assert_eq!(real(-0.25), "-0.25");
        assert_eq!(real(1e-7), "0.0000001");
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string("it's"), "'it''s'");
        assert_eq!(string("a\\b"), "'a\\\\b'");
        assert_eq!(string("Café"), "'Caf\\X2\\00E9\\X0\\'");
        assert_eq!(
            string("PAINTED,\nTAPERED"),
            "'PAINTED,\\X2\\000A\\X0\\TAPERED'"
        );
    }
}
