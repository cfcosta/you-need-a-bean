// Local addition vs upstream 2.6.0: the parser's error type, and the record of
// where a parse got furthest. See VENDOR.md.
//
// nom's own error keeps one position: wherever the last alternative gave up.
// For a line no rule claims that is the start of the line, whatever went wrong
// on it, and a failure swallowed on the way (by `opt`, `many0` or `iterator`)
// leaves no trace at all. So a posting whose currency is `US$` came out as
// "error at the start of the next line", with nothing to say why.
//
// Instead, every failure any rule reports is weighed here, and the one that
// got furthest into the input wins: the classic heuristic for PEG parsers,
// since the rule that read the most is the one the author most likely meant.
// `context` labels say what that rule was looking for. The first label to
// reach the furthest position wins, which is the innermost one, because
// labels are attached as a failure unwinds.
//
// The record lives in a thread-local because nom threads only the input and
// the error through a parse, and the failure that matters is often in neither
// by the time the parse gives up. `Iter` saves and restores it around every
// entry, so two iterators interleaved on one thread cannot see each other's.

use std::cell::Cell;

use nom::{
    error::{ContextError, ErrorKind, FromExternalError, ParseError},
    Parser,
};

use crate::Span;

/// The furthest point a parse reached before a rule rejected it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Furthest {
    pub(crate) offset: usize,
    pub(crate) line: u32,
    pub(crate) expected: Option<&'static str>,
    /// How much input the failing rule read before rejecting it, when it read
    /// a whole token and then found it wanting: the `13` of a month.
    pub(crate) len: Option<usize>,
}

thread_local! {
    static FURTHEST: Cell<Option<Furthest>> = const { Cell::new(None) };
}

/// Replace the record, returning what it held.
pub(crate) fn swap(furthest: Option<Furthest>) -> Option<Furthest> {
    FURTHEST.with(|cell| cell.replace(furthest))
}

fn reached(input: &Span<'_>, len: Option<usize>) {
    FURTHEST.with(|cell| {
        let offset = input.location_offset();
        match cell.get() {
            Some(f) if offset < f.offset => {}
            Some(f) if offset == f.offset => {
                if f.len.is_none() {
                    cell.set(Some(Furthest { len, ..f }));
                }
            }
            _ => cell.set(Some(Furthest {
                offset,
                line: input.location_line(),
                expected: None,
                len,
            })),
        }
    });
}

/// Read a token with `read`, then keep it only if `check` makes something of
/// it. A rejected token is reported whole, so an error can underline exactly
/// what was read rather than guess where it ends.
pub(crate) fn checked<'a, O>(
    mut read: impl Parser<Span<'a>, Output = Span<'a>, Error = Error<'a>>,
    check: impl Fn(&str) -> Option<O>,
) -> impl FnMut(Span<'a>) -> nom::IResult<Span<'a>, O, Error<'a>> {
    move |input| {
        let (rest, token) = read.parse(input)?;
        if let Some(value) = check(token.fragment()) {
            Ok((rest, value))
        } else {
            reached(&input, Some(token.fragment().len()));
            Err(nom::Err::Error(Error { input }))
        }
    }
}

fn label(input: &Span<'_>, expected: &'static str) {
    FURTHEST.with(|cell| {
        if let Some(f) = cell.get()
            && f.offset == input.location_offset()
            && f.expected.is_none()
        {
            cell.set(Some(Furthest {
                expected: Some(expected),
                ..f
            }));
        }
    });
}

/// A failure that is not about how far the parse got — dividing by zero, a
/// word that is no keyword — and must be reported where it is, as it is.
pub(crate) fn reject<'a>(input: Span<'a>, expected: &'static str) -> nom::Err<Error<'a>> {
    FURTHEST.with(|cell| {
        cell.set(Some(Furthest {
            offset: input.location_offset(),
            line: input.location_line(),
            expected: Some(expected),
            len: None,
        }));
    });
    nom::Err::Failure(Error { input })
}

/// The error every parser in this crate returns.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Error<'a> {
    pub(crate) input: Span<'a>,
}

impl<'a> ParseError<Span<'a>> for Error<'a> {
    fn from_error_kind(input: Span<'a>, _: ErrorKind) -> Self {
        reached(&input, None);
        Self { input }
    }

    fn append(_: Span<'a>, _: ErrorKind, other: Self) -> Self {
        other
    }

    fn or(self, other: Self) -> Self {
        if other.input.location_offset() >= self.input.location_offset() {
            other
        } else {
            self
        }
    }
}

impl<'a> ContextError<Span<'a>> for Error<'a> {
    fn add_context(_: Span<'a>, expected: &'static str, other: Self) -> Self {
        label(&other.input, expected);
        other
    }
}

impl<'a, E> FromExternalError<Span<'a>, E> for Error<'a> {
    fn from_external_error(input: Span<'a>, kind: ErrorKind, _: E) -> Self {
        Self::from_error_kind(input, kind)
    }
}
