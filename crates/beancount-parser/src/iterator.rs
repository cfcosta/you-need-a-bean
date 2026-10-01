use std::collections::{HashMap, HashSet};

use nom::Input;

use crate::{
    failure::{self, Furthest}, metadata, DirectiveContent, Entry, Error, IResult, RawEntry, Span,
    Tag,
};

// Local patch vs upstream 2.6.0: the iterator drives `entry` itself rather than
// through nom's `iterator`, which stops for good at the first failure and
// keeps no record of it. This one reports each failure where the parse got
// furthest, then resumes at the next line that starts a new entry, so a run
// reports every error in a file rather than the first. See VENDOR.md.
pub(crate) struct Iter<'i, D, F> {
    source: &'i str,
    rest: Option<Span<'i>>,
    entry: F,
    // How far the last entry's parse looked past where it stopped. A
    // transaction tries the line after its last posting as one more posting,
    // and when that line then fails as an entry of its own, the attempt that
    // got furthest into it is the one that says why.
    furthest: Option<Furthest>,
    // The last multi-line string in the entry before this one. A string
    // missing its closing quote can end cleanly at the next one, so the entry
    // holding it parses and the one after fails instead.
    multiline: Option<(usize, usize)>,
    tag_stack: HashSet<Tag>,
    // Local addition vs upstream 2.6.0, mirroring `tag_stack`: `pushmeta` and
    // `popmeta` nest, so each key holds a stack and the innermost push wins.
    meta_stack: HashMap<metadata::Key, Vec<metadata::Value<D>>>,
}

impl<'i, D, F> Iter<'i, D, F> {
    pub(crate) fn new(source: &'i str, entry: F) -> Self {
        Self {
            source,
            rest: Some(Span::new(source)),
            entry,
            furthest: None,
            multiline: None,
            tag_stack: HashSet::new(),
            meta_stack: HashMap::new(),
        }
    }
}

impl<'i, D: Clone, F> Iterator for Iter<'i, D, F>
where
    F: FnMut(Span<'i>) -> IResult<'i, RawEntry<D>>,
{
    type Item = Result<Entry<D>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let input = self.rest?;
            if input.fragment().is_empty() {
                self.rest = None;
                return None;
            }
            let start = input.location_offset();
            let outer = failure::swap(self.furthest.filter(|f| f.offset > start));
            let outer_multiline = failure::swap_multiline(None);
            let result = (self.entry)(input);
            self.furthest = failure::swap(outer);
            let multiline = failure::swap_multiline(outer_multiline);
            let before = std::mem::replace(&mut self.multiline, multiline);
            if result.is_err() {
                self.multiline = multiline.or(before);
            }
            let entry = match result {
                Ok((rest, entry)) if rest.location_offset() > start => {
                    self.rest = Some(rest);
                    entry
                }
                Ok((rest, _)) => return Some(Err(self.fail(input, rest))),
                Err(nom::Err::Error(e) | nom::Err::Failure(e)) => {
                    return Some(Err(self.fail(input, e.input)));
                }
                Err(nom::Err::Incomplete(_)) => return Some(Err(self.fail(input, input))),
            };
            match entry {
                RawEntry::Directive(mut d) => {
                    if let DirectiveContent::Transaction(trx) = &mut d.content {
                        trx.tags.extend(self.tag_stack.iter().cloned());
                    }
                    for (key, stack) in &self.meta_stack {
                        if let Some(value) = stack.last() {
                            d.metadata
                                .entry(key.clone())
                                .or_insert_with(|| value.clone());
                        }
                    }
                    return Some(Ok(Entry::Directive(d)));
                }
                RawEntry::Option(o) => {
                    return Some(Ok(Entry::Option(o)));
                }
                RawEntry::Include(path) => {
                    return Some(Ok(Entry::Include(path)));
                }
                RawEntry::Plugin(plugin) => {
                    return Some(Ok(Entry::Plugin(plugin)));
                }
                RawEntry::PushTag(tag) => {
                    self.tag_stack.insert(tag);
                }
                RawEntry::PopTag(tag) => {
                    self.tag_stack.remove(&tag);
                }
                RawEntry::PushMeta(key, value) => {
                    self.meta_stack.entry(key).or_default().push(value);
                }
                RawEntry::PopMeta(key) => {
                    if let Some(stack) = self.meta_stack.get_mut(&key) {
                        stack.pop();
                    }
                }
                RawEntry::Comment => (),
            }
        }
    }
}

impl<'i, D, F> Iter<'i, D, F> {
    /// The error for an entry starting at `input` that failed at `stopped`,
    /// and where to resume after it.
    fn fail(&mut self, input: Span<'i>, stopped: Span<'_>) -> Error {
        let failure = self
            .furthest
            .take()
            .filter(|f| f.offset >= stopped.location_offset())
            .unwrap_or(Furthest {
                offset: stopped.location_offset(),
                line: stopped.location_line(),
                expected: None,
                len: None,
            });
        self.rest = Some(resume(input, failure.offset));
        Error::at(self.source, failure.offset, failure.line, failure.expected)
            .with_token_len(failure.len)
            .with_multiline_string(self.multiline.take())
    }
}

/// Where parsing picks up after an error at `offset`: past the rest of that
/// line, and past every indented or blank line after it, since those belong to
/// the entry that just failed. The next line starting in column 1 starts
/// something new.
fn resume(input: Span<'_>, offset: usize) -> Span<'_> {
    let text = input.fragment();
    let line_end = |at: usize| text[at..].find('\n').map_or(text.len(), |i| at + i + 1);
    let mut at = line_end(offset - input.location_offset());
    while at < text.len() && text[at..].starts_with([' ', '\t', '\r', '\n']) {
        at = line_end(at);
    }
    input.take_from(at)
}
