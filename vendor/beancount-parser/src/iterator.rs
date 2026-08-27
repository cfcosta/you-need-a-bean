use std::collections::{HashMap, HashSet};

use nom::{combinator::ParserIterator, Finish};

use crate::{
    metadata, DirectiveContent, Entry, Error, RawEntry, Span, Tag,
};

type InnerIter<'i, F> = ParserIterator<Span<'i>, nom::error::Error<Span<'i>>, F>;

pub(crate) struct Iter<'i, D, F> {
    source: &'i str,
    inner: Option<InnerIter<'i, F>>,
    tag_stack: HashSet<Tag>,
    // Local addition vs upstream 2.6.0, mirroring `tag_stack`: `pushmeta` and
    // `popmeta` nest, so each key holds a stack and the innermost push wins.
    meta_stack: HashMap<metadata::Key, Vec<metadata::Value<D>>>,
}

impl<'i, D, F> Iter<'i, D, F> {
    pub(crate) fn new(source: &'i str, value: InnerIter<'i, F>) -> Self {
        Self {
            source,
            inner: Some(value),
            tag_stack: HashSet::new(),
            meta_stack: HashMap::new(),
        }
    }
}

impl<'i, D: Clone, F> Iterator for Iter<'i, D, F>
where
    for<'a> &'a mut InnerIter<'i, F>: Iterator<Item = RawEntry<D>>,
{
    type Item = Result<Entry<D>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        let inner = self.inner.as_mut()?;
        for entry in inner {
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
        match self.inner.take().unwrap().finish().finish() {
            Ok((rest, ())) if rest.fragment().is_empty() => None,
            Ok((input, ())) | Err(nom::error::Error { input, .. }) => {
                Some(Err(Error::new(self.source, input)))
            }
        }
    }
}
