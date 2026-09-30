//! Form selection (spec §9, §10) and text output for a [`Node`] tree.
//!
//! The writer walks the tree once. At every array or object it classifies
//! the value into a form ([`ArrayForm`], [`ObjectForm`]) from its shape and
//! position, then writes it directly into the output `String`.
//!
//! Depth model (§10): a field written on a list item's hyphen line at depth
//! `d` stands at depth `d + 1`, so every field is written by
//! [`Writer::field`] with the depth it stands at, and the scope it opens
//! always has its content at `depth + 1`, whether the field starts its own
//! line or follows a hyphen.

use super::ir::{Entry, Node, Text};
use super::number;
use super::EncodeConfig;
use crate::lexical;
use std::collections::HashMap;

/// Enough spaces for most indentation in a single `push_str`.
const SPACES: &str = "                                                                ";

/// Placeholder for a cell that detection guarantees is present.
static NULL: Node = Node::Null;

/// Key sets up to this size are searched linearly; larger ones are hashed.
const LINEAR_KEYS: usize = 16;

/// Where an array appears. Tabular form needs a key or the document root
/// (§6: a keyless fields-bearing header is valid only at the root).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrayPosition {
    /// An object field or the document root.
    Keyed,
    /// Directly as a list item (`- [N]: ...`).
    ListItem,
}

/// The form an array is written in.
enum ArrayForm<'n> {
    /// `key: []`, `[]` at the root, `- [0]:` as a list item.
    Empty,
    /// §9.1: all elements are primitives.
    Inline,
    /// §9.3: uniform non-empty objects.
    Tabular(Schema<'n>),
    /// §9.2 / §9.4: anything else.
    List,
}

/// The form an object in field or root position is written in.
enum ObjectForm<'n> {
    /// `key:` alone, or the empty document at the root.
    Empty,
    /// §8: one field per line.
    Nested,
    /// §9.5: two or more uniform non-empty object values.
    KeyedTabular(Schema<'n>),
}

/// The field structure shared by all rows of a tabular array or keyed
/// tabular object: the first row's keys, in its encounter order, each with a
/// nested field group when its column is nested-uniform.
struct Schema<'n> {
    /// The first row, whose key order the header follows.
    first: &'n [Entry],
    fields: Vec<Field<'n>>,
    index: KeyIndex<'n>,
}

struct Field<'n> {
    key: &'n Text,
    group: Option<Schema<'n>>,
}

/// Finds a key's position in the first row, for rows whose keys come in a
/// different order.
enum KeyIndex<'n> {
    Linear,
    Hashed(HashMap<&'n str, usize>),
}

/// What a column holds, decided by the first row and required of all rows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Column {
    Primitive,
    Nested,
}

/// Whether two keys are the same string; derived struct field names are
/// usually the very same `&'static str`, which is checked first.
fn same_key(a: &Text, b: &Text, arena: &str) -> bool {
    match (a, b) {
        (Text::Static(x), Text::Static(y)) if std::ptr::eq(*x, *y) => true,
        _ => a.resolve(arena) == b.resolve(arena),
    }
}

impl<'n> KeyIndex<'n> {
    /// Indexes the first row's keys; `None` if it has duplicate keys, which
    /// can never be written as a field list (§9.3: duplicates are a header
    /// defect).
    fn build(first: &'n [Entry], arena: &'n str) -> Option<Self> {
        if first.len() <= LINEAR_KEYS {
            for (i, (key, _)) in first.iter().enumerate() {
                if first[..i].iter().any(|(k, _)| same_key(k, key, arena)) {
                    return None;
                }
            }
            Some(KeyIndex::Linear)
        } else {
            let mut map = HashMap::with_capacity(first.len());
            for (i, (key, _)) in first.iter().enumerate() {
                if map.insert(key.resolve(arena), i).is_some() {
                    return None;
                }
            }
            Some(KeyIndex::Hashed(map))
        }
    }

    fn find(&self, first: &[Entry], key: &Text, arena: &str) -> Option<usize> {
        match self {
            KeyIndex::Linear => first.iter().position(|(k, _)| same_key(k, key, arena)),
            KeyIndex::Hashed(map) => map.get(key.resolve(arena)).copied(),
        }
    }
}

/// Whether `row` has exactly `first`'s keys in the same order.
fn same_order(first: &[Entry], row: &[Entry], arena: &str) -> bool {
    first
        .iter()
        .zip(row)
        .all(|((a, _), (b, _))| same_key(a, b, arena))
}

/// Whether `value` fits `column`; a nested value's entries join the
/// column's sub-rows.
fn accept<'n>(column: Column, value: &'n Node, group: &mut Vec<&'n [Entry]>) -> bool {
    match column {
        Column::Primitive => value.is_primitive(),
        Column::Nested => match value.as_nonempty_object() {
            Some(entries) => {
                group.push(entries);
                true
            }
            None => false,
        },
    }
}

/// Tabular detection (§9.3), shared by keyed tabular detection (§9.5).
///
/// `rows` are the entries of non-empty objects. Succeeds when all rows have
/// the same key set (in any order) and every column is uniform-primitive or
/// nested-uniform, recursively. Runs in time linear in the number of cells
/// when rows share the first row's key order, and still correctly (by set
/// comparison) when they do not.
fn detect<'n>(rows: &[&'n [Entry]], arena: &'n str) -> Option<Schema<'n>> {
    let first = *rows.first()?;
    let width = first.len();
    let index = KeyIndex::build(first, arena)?;

    let mut columns = Vec::with_capacity(width);
    for (_, value) in first {
        columns.push(match value {
            v if v.is_primitive() => Column::Primitive,
            Node::Object(e) if !e.is_empty() => Column::Nested,
            // An array or empty object disqualifies the column.
            _ => return None,
        });
    }

    // Sub-rows of every nested column, collected column by column.
    let mut groups: Vec<Vec<&'n [Entry]>> = columns
        .iter()
        .map(|c| match c {
            Column::Nested => Vec::with_capacity(rows.len()),
            Column::Primitive => Vec::new(),
        })
        .collect();

    let mut seen = Vec::new();
    for &row in rows {
        if row.len() != width {
            return None;
        }
        if same_order(first, row, arena) {
            for (column, (_, value)) in row.iter().enumerate() {
                if !accept(columns[column], value, &mut groups[column]) {
                    return None;
                }
            }
        } else {
            // Same key set in another order: map every key to a distinct
            // column of the first row.
            seen.clear();
            seen.resize(width, false);
            for (key, value) in row {
                let column = index.find(first, key, arena)?;
                let repeated = std::mem::replace(&mut seen[column], true);
                if repeated || !accept(columns[column], value, &mut groups[column]) {
                    return None;
                }
            }
        }
    }

    let mut fields = Vec::with_capacity(width);
    for (((key, _), column), group_rows) in first.iter().zip(&columns).zip(&groups) {
        let group = match column {
            Column::Primitive => None,
            Column::Nested => Some(detect(group_rows, arena)?),
        };
        fields.push(Field { key, group });
    }
    Some(Schema {
        first,
        fields,
        index,
    })
}

/// Tabular detection for array elements.
fn detect_array<'n>(items: &'n [Node], arena: &'n str) -> Option<Schema<'n>> {
    let rows = items
        .iter()
        .map(Node::as_nonempty_object)
        .collect::<Option<Vec<_>>>()?;
    detect(&rows, arena)
}

/// Keyed tabular detection (§9.5) for an object in field or root position.
fn detect_keyed<'n>(entries: &'n [Entry], arena: &'n str) -> Option<Schema<'n>> {
    if entries.len() < 2 {
        return None;
    }
    let rows = entries
        .iter()
        .map(|(_, v)| v.as_nonempty_object())
        .collect::<Option<Vec<_>>>()?;
    detect(&rows, arena)
}

fn classify_array<'n>(items: &'n [Node], position: ArrayPosition, arena: &'n str) -> ArrayForm<'n> {
    if items.is_empty() {
        ArrayForm::Empty
    } else if items.iter().all(Node::is_primitive) {
        ArrayForm::Inline
    } else if position == ArrayPosition::Keyed {
        detect_array(items, arena).map_or(ArrayForm::List, ArrayForm::Tabular)
    } else {
        ArrayForm::List
    }
}

fn classify_object<'n>(entries: &'n [Entry], arena: &'n str) -> ObjectForm<'n> {
    if entries.is_empty() {
        ObjectForm::Empty
    } else {
        detect_keyed(entries, arena).map_or(ObjectForm::Nested, ObjectForm::KeyedTabular)
    }
}

/// Writes one document into `out`.
pub(crate) struct Writer<'w, 'n> {
    out: &'w mut String,
    arena: &'n str,
    config: &'w EncodeConfig,
}

impl<'w, 'n> Writer<'w, 'n> {
    pub(crate) fn new(out: &'w mut String, arena: &'n str, config: &'w EncodeConfig) -> Self {
        Writer { out, arena, config }
    }

    /// Writes one field of a root object in nested form (§8) whose document
    /// began at output length `start`: every field but the first starts a
    /// new line.
    pub(crate) fn root_field(&mut self, start: usize, key: &Text, value: &'n Node) {
        if self.out.len() > start {
            self.newline(0);
        }
        self.field(key, value, 0);
    }

    /// Writes a string as a whole document (a root primitive, §5).
    pub(crate) fn root_string(&mut self, s: &str) {
        self.string(s);
    }

    /// Writes `root` as a whole document (§5): no leading or trailing
    /// newline.
    pub(crate) fn document(&mut self, root: &'n Node) {
        match root {
            Node::Array(items) => match classify_array(items, ArrayPosition::Keyed, self.arena) {
                ArrayForm::Empty => self.out.push_str("[]"),
                form => self.array(items, form, 0),
            },
            Node::Object(entries) => match classify_object(entries, self.arena) {
                ObjectForm::Empty => {}
                ObjectForm::Nested => {
                    for (i, (key, value)) in entries.iter().enumerate() {
                        if i > 0 {
                            self.newline(0);
                        }
                        self.field(key, value, 0);
                    }
                }
                ObjectForm::KeyedTabular(schema) => self.keyed(entries, &schema, 0),
            },
            primitive => self.primitive(primitive),
        }
    }

    /// Starts a new line at `depth`.
    fn newline(&mut self, depth: usize) {
        self.out.push('\n');
        let mut n = depth * self.config.indent();
        while n > 0 {
            let chunk = n.min(SPACES.len());
            self.out.push_str(&SPACES[..chunk]);
            n -= chunk;
        }
    }

    fn delimiter(&self) -> char {
        self.config.delimiter_char()
    }

    /// §7.3: keys, entry keys, and field names.
    fn key(&mut self, key: &Text) {
        let key = key.resolve(self.arena);
        if lexical::is_unquoted_key(key) {
            self.out.push_str(key);
        } else {
            lexical::write_quoted(self.out, key);
        }
    }

    /// §7.2 with delimiter-aware quoting. Every header this encoder emits
    /// declares the document delimiter, so the active delimiter and the
    /// document delimiter (§11.1) are always the same.
    fn string(&mut self, s: &str) {
        if lexical::value_needs_quotes(s, self.config.delimiter()) {
            lexical::write_quoted(self.out, s);
        } else {
            self.out.push_str(s);
        }
    }

    fn primitive(&mut self, node: &Node) {
        match node {
            Node::Null => self.out.push_str("null"),
            Node::Bool(true) => self.out.push_str("true"),
            Node::Bool(false) => self.out.push_str("false"),
            Node::Int(v) => number::push_i64(self.out, *v),
            Node::UInt(v) => number::push_u64(self.out, *v),
            Node::Float(v) => number::push_float(self.out, *v),
            Node::Float32(v) => number::push_float(self.out, *v),
            Node::Str(s) => self.string(s.resolve(self.arena)),
            Node::BigInt(digits) => self.out.push_str(digits.resolve(self.arena)),
            // Callers only pass primitives; stay total regardless.
            Node::Array(_) | Node::Object(_) => self.out.push_str("null"),
        }
    }

    /// `[N<sym>]` or, keyed, `[N:<sym>]` (§6).
    fn bracket(&mut self, len: usize, keyed: Keyed) {
        self.out.push('[');
        number::push_u64(self.out, len as u64);
        if keyed == Keyed::Yes {
            self.out.push(':');
        }
        self.out.push_str(self.config.header_symbol());
        self.out.push(']');
    }

    /// `{f1,f2{g1,g2}}` (§6, §9.3).
    fn field_list(&mut self, schema: &Schema<'n>) {
        self.out.push('{');
        for (i, field) in schema.fields.iter().enumerate() {
            if i > 0 {
                self.out.push(self.delimiter());
            }
            self.key(field.key);
            if let Some(group) = &field.group {
                self.field_list(group);
            }
        }
        self.out.push('}');
    }

    /// Writes a row's leaf values in depth-first header order (§9.3).
    fn cells(&mut self, row: &'n [Entry], schema: &Schema<'n>, first: &mut bool) {
        if same_order(schema.first, row, self.arena) {
            for (field, (_, value)) in schema.fields.iter().zip(row) {
                self.cell(field, value, first);
            }
        } else {
            // Same key set in another order (checked by detection): place
            // each value in its header column first.
            let mut slots: Vec<&'n Node> = vec![&NULL; schema.fields.len()];
            for (key, value) in row {
                if let Some(column) = schema.index.find(schema.first, key, self.arena) {
                    slots[column] = value;
                }
            }
            for (field, value) in schema.fields.iter().zip(slots) {
                self.cell(field, value, first);
            }
        }
    }

    fn cell(&mut self, field: &Field<'n>, value: &'n Node, first: &mut bool) {
        match (&field.group, value) {
            (Some(group), Node::Object(sub)) => self.cells(sub, group, first),
            _ => {
                if !std::mem::replace(first, false) {
                    self.out.push(self.delimiter());
                }
                self.primitive(value);
            }
        }
    }

    /// Writes a field standing at `depth`; the line is already started.
    fn field(&mut self, key: &Text, value: &'n Node, depth: usize) {
        self.key(key);
        match value {
            Node::Array(items) => match classify_array(items, ArrayPosition::Keyed, self.arena) {
                ArrayForm::Empty => self.out.push_str(": []"),
                form => self.array(items, form, depth),
            },
            Node::Object(entries) => match classify_object(entries, self.arena) {
                ObjectForm::Empty => self.out.push(':'),
                ObjectForm::Nested => {
                    self.out.push(':');
                    for (key, value) in entries {
                        self.newline(depth + 1);
                        self.field(key, value, depth + 1);
                    }
                }
                ObjectForm::KeyedTabular(schema) => self.keyed(entries, &schema, depth),
            },
            primitive => {
                self.out.push_str(": ");
                self.primitive(primitive);
            }
        }
    }

    /// Writes a non-empty array's header (after its key, if any) and body;
    /// the header stands at `depth`, its content at `depth + 1`.
    fn array(&mut self, items: &'n [Node], form: ArrayForm<'n>, depth: usize) {
        self.bracket(items.len(), Keyed::No);
        match form {
            ArrayForm::Empty | ArrayForm::Inline => {
                self.out.push(':');
                for (i, item) in items.iter().enumerate() {
                    self.out.push(if i == 0 { ' ' } else { self.delimiter() });
                    self.primitive(item);
                }
            }
            ArrayForm::Tabular(schema) => {
                self.field_list(&schema);
                self.out.push(':');
                for item in items {
                    if let Node::Object(row) = item {
                        self.newline(depth + 1);
                        self.cells(row, &schema, &mut true);
                    }
                }
            }
            ArrayForm::List => {
                self.out.push(':');
                for item in items {
                    self.newline(depth + 1);
                    self.list_item(item, depth + 1);
                }
            }
        }
    }

    /// Writes a keyed tabular object's header (after its key, if any) and
    /// entry rows (§9.5).
    fn keyed(&mut self, entries: &'n [Entry], schema: &Schema<'n>, depth: usize) {
        self.bracket(entries.len(), Keyed::Yes);
        self.field_list(schema);
        self.out.push(':');
        for (key, value) in entries {
            if let Node::Object(row) = value {
                self.newline(depth + 1);
                self.key(key);
                self.out.push_str(": ");
                self.cells(row, schema, &mut true);
            }
        }
    }

    /// Writes one list item (§9.4, §10) whose hyphen stands at `depth`; the
    /// line is already started.
    fn list_item(&mut self, item: &'n Node, depth: usize) {
        match item {
            Node::Array(items) => {
                match classify_array(items, ArrayPosition::ListItem, self.arena) {
                    // §9.2: never `- []` for a list-item inner array.
                    ArrayForm::Empty => {
                        self.out.push_str("- ");
                        self.bracket(0, Keyed::No);
                        self.out.push(':');
                    }
                    form => {
                        self.out.push_str("- ");
                        self.array(items, form, depth);
                    }
                }
            }
            Node::Object(entries) => match entries.split_first() {
                None => self.out.push('-'),
                Some(((key, value), rest)) => {
                    self.out.push_str("- ");
                    self.field(key, value, depth + 1);
                    for (key, value) in rest {
                        self.newline(depth + 1);
                        self.field(key, value, depth + 1);
                    }
                }
            },
            primitive => {
                self.out.push_str("- ");
                self.primitive(primitive);
            }
        }
    }
}

/// Whether a bracket segment is keyed (`[N:]`, §9.5).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Keyed {
    Yes,
    No,
}
