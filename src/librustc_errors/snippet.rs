// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Code for annotating snippets.

use syntax_pos::{Span, FileMap};
use CodeMapper;
use std::rc::Rc;
use Level;

#[derive(Clone)]
pub struct SnippetData {
    codemap: Rc<CodeMapper>,
    files: Vec<FileInfo>,
}

#[derive(Clone)]
pub(crate) struct FileInfo {
    file: Rc<FileMap>,

    /// The "primary file", if any, gets a `-->` marker instead of
    /// `>>>`, and has a line-number/column printed and not just a
    /// filename.  It appears first in the listing. It is known to
    /// contain at least one primary span, though primary spans (which
    /// are designated with `^^^`) may also occur in other files.
    primary_span: Option<Span>,

    lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) line_index: usize,
    pub(crate) annotations: Vec<Annotation>,
}


#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) struct MultilineAnnotation {
    pub(crate) depth: usize,
    pub(crate) line_start: usize,
    pub(crate) line_end: usize,
    pub(crate) start_col: usize,
    pub(crate) end_col: usize,
    pub(crate) is_primary: bool,
    pub(crate) label: Option<String>,
}

impl MultilineAnnotation {
    pub(crate) fn increase_depth(&mut self) {
        self.depth += 1;
    }

    pub(crate) fn as_start(&self) -> Annotation {
        Annotation {
            start_col: self.start_col,
            end_col: self.start_col + 1,
            is_primary: self.is_primary,
            label: None,
            annotation_type: AnnotationType::MultilineStart(self.depth)
        }
    }

    pub(crate) fn as_end(&self) -> Annotation {
        Annotation {
            start_col: self.end_col - 1,
            end_col: self.end_col,
            is_primary: self.is_primary,
            label: self.label.clone(),
            annotation_type: AnnotationType::MultilineEnd(self.depth)
        }
    }

    pub(crate) fn as_line(&self) -> Annotation {
        Annotation {
            start_col: 0,
            end_col: 0,
            is_primary: self.is_primary,
            label: None,
            annotation_type: AnnotationType::MultilineLine(self.depth)
        }
    }
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) enum AnnotationType {
    /// Annotation under a single line of code
    Singleline,

    /// Annotation enclosing the first and last character of a multiline span
    Multiline(MultilineAnnotation),

    // The Multiline type above is replaced with the following three in order
    // to reuse the current label drawing code.
    //
    // Each of these corresponds to one part of the following diagram:
    //
    //     x |   foo(1 + bar(x,
    //       |  _________^              < MultilineStart
    //     x | |             y),        < MultilineLine
    //       | |______________^ label   < MultilineEnd
    //     x |       z);
    /// Annotation marking the first character of a fully shown multiline span
    MultilineStart(usize),
    /// Annotation marking the last character of a fully shown multiline span
    MultilineEnd(usize),
    /// Line at the left enclosing the lines of a fully shown multiline span
    // Just a placeholder for the drawing algorithm, to know that it shouldn't skip the first 4
    // and last 2 lines of code. The actual line is drawn in `emit_message_default` and not in
    // `draw_multiline_line`.
    MultilineLine(usize),
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub(crate) struct Annotation {
    /// Start column, 0-based indexing -- counting *characters*, not
    /// utf-8 bytes. Note that it is important that this field goes
    /// first, so that when we sort, we sort orderings by start
    /// column.
    pub(crate) start_col: usize,

    /// End column within the line (exclusive)
    pub(crate) end_col: usize,

    /// Is this annotation derived from primary span
    pub(crate) is_primary: bool,

    /// Optional label to display adjacent to the annotation.
    pub(crate) label: Option<String>,

    /// Is this a single line, multiline or multiline span minimized down to a
    /// smaller span.
    pub(crate) annotation_type: AnnotationType,
}

impl Annotation {
    /// Wether this annotation is a vertical line placeholder.
    pub(crate) fn is_line(&self) -> bool {
        if let AnnotationType::MultilineLine(_) = self.annotation_type {
            true
        } else {
            false
        }
    }

    pub(crate) fn is_multiline(&self) -> bool {
        match self.annotation_type {
            AnnotationType::Multiline(_) |
            AnnotationType::MultilineStart(_) |
            AnnotationType::MultilineLine(_) |
            AnnotationType::MultilineEnd(_) => true,
            _ => false,
        }
    }

    pub(crate) fn len(&self) -> usize {
        // Account for usize underflows
        if self.end_col > self.start_col {
            self.end_col - self.start_col
        } else {
            self.start_col - self.end_col
        }
    }

    pub(crate) fn has_label(&self) -> bool {
        if let Some(ref label) = self.label {
            // Consider labels with no text as effectively not being there
            // to avoid weird output with unnecessary vertical lines, like:
            //
            //     X | fn foo(x: u32) {
            //       | -------^------
            //       | |      |
            //       | |
            //       |
            //
            // Note that this would be the complete output users would see.
            label.len() > 0
        } else {
            false
        }
    }

    pub(crate) fn takes_space(&self) -> bool {
        // Multiline annotations always have to keep vertical space.
        match self.annotation_type {
            AnnotationType::MultilineStart(_) |
            AnnotationType::MultilineEnd(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(crate) struct StyledString {
    pub(crate) text: String,
    pub(crate) style: Style,
}

#[derive(Copy, Clone, Debug, PartialEq, RustcEncodable, RustcDecodable)]
pub enum Style {
    HeaderMsg,
    LineAndColumn,
    LineNumber,
    Quotation,
    UnderlinePrimary,
    UnderlineSecondary,
    LabelPrimary,
    LabelSecondary,
    NoStyle,
    Level(Level),
    Highlight,
}
