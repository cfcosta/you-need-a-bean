#![allow(clippy::module_name_repetitions)]
#![allow(deprecated)]
#![allow(unused_assignments)]

use std::{
    fmt::{Debug, Display},
    io,
    path::PathBuf,
};

#[cfg(feature = "miette")]
use miette::{Diagnostic, SourceSpan};

use crate::Span;

/// Error returned in case of invalid beancount syntax found
///
/// # Example
/// ```
/// # use beancount_parser::BeancountFile;
/// let result: Result<BeancountFile<f64>, beancount_parser::Error> = "2022-05-21 oops".parse();
/// assert!(result.is_err());
/// let error = result.unwrap_err();
/// assert_eq!(error.line_number(), 1);
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "miette", derive(Diagnostic))]
pub struct Error {
    #[cfg(feature = "miette")]
    #[source_code]
    src: String,
    #[cfg(feature = "miette")]
    #[label]
    span: SourceSpan,
    line_number: u32,
    // Local addition vs upstream 2.6.0: the byte offset the parse stopped at,
    // unconditionally. Upstream keeps it only under the `miette` feature, and
    // only as a `SourceSpan` inside a `Diagnostic` this crate derives itself.
    // Exposing the raw offset lets the caller build whatever diagnostic it
    // likes -- and decide how far past the offset to underline, which is a
    // question the parser has no answer to. See VENDOR.md.
    offset: usize,
    // Local addition vs upstream 2.6.0: what the parser was looking for at
    // `offset`, when a rule said. See VENDOR.md.
    expected: Option<&'static str>,
    // Local addition vs upstream 2.6.0: how long the rejected token at
    // `offset` is, when the rule read one whole. See VENDOR.md.
    token_len: Option<usize>,
    // Local addition vs upstream 2.6.0: a string that ran over a line break
    // just before the error, as (offset, length). Boxed because it is rare
    // and the error travels by value. See VENDOR.md.
    multiline_string: Option<Box<(usize, usize)>>,
}

impl Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Error")
            .field("line_number", &self.line_number())
            .finish()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid beancount syntax at line: {}", self.line_number)
    }
}

impl std::error::Error for Error {}

impl Error {
    pub(crate) fn new(src: impl Into<String>, span: Span<'_>) -> Self {
        Self::at(src, span.location_offset(), span.location_line(), None)
    }

    #[cfg(not(feature = "miette"))]
    pub(crate) fn at(
        _: impl Into<String>,
        offset: usize,
        line_number: u32,
        expected: Option<&'static str>,
    ) -> Self {
        Self {
            line_number,
            offset,
            expected,
            token_len: None,
            multiline_string: None,
        }
    }

    #[cfg(feature = "miette")]
    pub(crate) fn at(
        src: impl Into<String>,
        offset: usize,
        line_number: u32,
        expected: Option<&'static str>,
    ) -> Self {
        Self {
            src: src.into(),
            span: offset.into(),
            line_number,
            offset,
            expected,
            token_len: None,
            multiline_string: None,
        }
    }

    pub(crate) fn with_multiline_string(self, multiline_string: Option<(usize, usize)>) -> Self {
        Self {
            multiline_string: multiline_string.map(Box::new),
            ..self
        }
    }

    pub(crate) fn with_token_len(self, token_len: Option<usize>) -> Self {
        Self { token_len, ..self }
    }

    /// Line number at which the error was found in the input
    #[must_use]
    pub fn line_number(&self) -> u32 {
        self.line_number
    }

    /// Byte offset at which the error was found in the input
    ///
    /// Zero-based, and counted in bytes rather than characters, so it can index
    /// the input directly. Pair it with [`Error::line_number`] to point at the
    /// failure: the offset says where, the line number says where to say it.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// What the parser was looking for at [`Error::offset`], if a rule said
    ///
    /// A noun phrase meant to follow "expected": `"an account"`, `"the end of
    /// the line"`, `"a day from 01 to 31"`. `None` when no rule had anything
    /// more specific to say than that the input stopped making sense.
    #[must_use]
    pub fn expected(&self) -> Option<&'static str> {
        self.expected
    }

    /// Length in bytes of the token at [`Error::offset`] that a rule read and
    /// rejected, if it read one whole
    ///
    /// The `13` of month 13, the `10..0` of a number that is not one. `None`
    /// when the parser stopped on something no rule could start reading, so
    /// where the trouble ends is the caller's guess.
    #[must_use]
    pub fn token_len(&self) -> Option<usize> {
        self.token_len
    }

    /// A string that ran over a line break in the entry that failed, or the
    /// one just before it, as the byte offset of its opening quote and its
    /// length up to and including the closing one
    ///
    /// Beancount strings may span lines, so a missing closing quote is not
    /// an error where it is missing: the string runs on to the next `"`, and
    /// the parse fails somewhere past it. This is where to point back to.
    #[must_use]
    pub fn multiline_string(&self) -> Option<(usize, usize)> {
        self.multiline_string.as_deref().copied()
    }
}

/// Error returned when reading a beancount file from disk
#[allow(missing_docs)]
#[derive(Debug)]
pub struct ReadFileErrorV2 {
    path: PathBuf,
    pub(crate) error: ReadFileErrorContent,
}

impl ReadFileErrorV2 {
    pub(crate) fn from_io(path: PathBuf, err: io::Error) -> Self {
        Self {
            path,
            error: ReadFileErrorContent::Io(err),
        }
    }

    pub(crate) fn from_syntax(path: PathBuf, err: Error) -> Self {
        Self {
            path,
            error: ReadFileErrorContent::Syntax(err),
        }
    }
}

impl Display for ReadFileErrorV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.error {
            ReadFileErrorContent::Io(err) => {
                write!(f, "Cannot read {}: {}", self.path.display(), err)
            }
            ReadFileErrorContent::Syntax(err) => {
                write!(f, "Invalid syntax in {}: {}", self.path.display(), err)
            }
        }
    }
}

impl std::error::Error for ReadFileErrorV2 {}

/// Content of the error returned when reading a beancount file from disk
#[allow(missing_docs)]
#[derive(Debug)]
pub(crate) enum ReadFileErrorContent {
    Io(std::io::Error),
    Syntax(Error),
}

/// Content of the error returned when reading a beancount file from disk
#[allow(missing_docs)]
#[derive(Debug)]
#[cfg_attr(feature = "miette", derive(Diagnostic))]
#[deprecated(since = "2.4.0", note = "use `ReadFileErrorV2 instead`")]
pub enum ReadFileError {
    Io(std::io::Error),
    Syntax(Error),
}

impl From<std::io::Error> for ReadFileError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<Error> for ReadFileError {
    fn from(value: Error) -> Self {
        Self::Syntax(value)
    }
}

impl Display for ReadFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadFileError::Io(_) => write!(f, "IO error"),
            ReadFileError::Syntax(_) => write!(f, "Syntax error"),
        }
    }
}

impl std::error::Error for ReadFileError {}

/// Error that may be returned by the various `TryFrom`/`TryInto` implementation
/// to signify that the value cannot be converted to the desired type
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ConversionError;

impl Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cannot convert to the desired type")
    }
}

impl std::error::Error for ConversionError {}
