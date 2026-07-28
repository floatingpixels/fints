//! Internal FinTS 3.0 delimiter-syntax codec.
//!
//! This representation is deliberately crate-private. Public callers interact with
//! concrete operations rather than a universal segment tree.

use std::borrow::Cow;

use encoding_rs::mem;
use thiserror::Error;

mod parser;

use parser::parse_segment_sequence;

const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_SEGMENTS: usize = 256;
const MAX_ELEMENTS_PER_SEGMENT: usize = 256;
const MAX_COMPONENTS_PER_ELEMENT: usize = 64;
const MAX_BINARY_BYTES: usize = MAX_MESSAGE_BYTES;
const SYNTAX_BYTES: &[u8] = b"+:'?@";

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Message {
    segments: Vec<Segment>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Segment {
    elements: Vec<Element>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Element {
    components: Vec<Value>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Value {
    Text(Vec<u8>),
    Binary(Vec<u8>),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("FinTS message is empty")]
    EmptyMessage,
    #[error("FinTS message exceeds the {limit}-byte safety limit")]
    MessageTooLarge { limit: usize },
    #[error("FinTS message contains more than {limit} segments")]
    TooManySegments { limit: usize },
    #[error("FinTS segment {segment} contains more than {limit} elements")]
    TooManyElements { segment: usize, limit: usize },
    #[error("FinTS element {element} in segment {segment} contains more than {limit} components")]
    TooManyComponents {
        segment: usize,
        element: usize,
        limit: usize,
    },
    #[error("FinTS escape at byte {offset} is incomplete")]
    IncompleteEscape { offset: usize },
    #[error("FinTS escape at byte {offset} does not precede a syntax character")]
    InvalidEscape { offset: usize },
    #[error("FinTS binary marker at byte {offset} is not at the start of a data element")]
    MisplacedBinaryMarker { offset: usize },
    #[error("FinTS binary length at byte {offset} is malformed")]
    InvalidBinaryLength { offset: usize },
    #[error("FinTS binary value at byte {offset} exceeds the {limit}-byte safety limit")]
    BinaryTooLarge { offset: usize, limit: usize },
    #[error("FinTS binary value at byte {offset} is truncated")]
    TruncatedBinary { offset: usize },
    #[error("FinTS binary value at byte {offset} is followed by non-delimiter data")]
    BinaryHasTrailingData { offset: usize },
    #[error("FinTS message does not end with a segment terminator")]
    MissingSegmentTerminator,
    #[error("FinTS segment {segment} has an invalid segment header")]
    InvalidSegmentHeader { segment: usize },
    #[error("FinTS segment number is {actual}, expected {expected}")]
    NonSequentialSegmentNumber { expected: u16, actual: u16 },
    #[error("FinTS message must start with HNHBK version 3 and end with HNHBS version 1")]
    InvalidMessageEnvelope,
    #[error("FinTS message length is {declared} bytes, actual length is {actual} bytes")]
    InvalidMessageLength { declared: usize, actual: usize },
    #[error("FinTS message header and trailer numbers differ")]
    MismatchedMessageNumber,
    #[error("FinTS security envelope is malformed")]
    InvalidSecurityEnvelope,
    #[error("text cannot be represented by the FinTS Latin-1 code set")]
    NonLatin1Text,
    #[error("FinTS message length cannot be represented in the 12-digit header field")]
    MessageLengthOverflow,
}

impl Message {
    pub(crate) fn new(segments: Vec<Segment>) -> Self {
        Self { segments }
    }

    pub(crate) fn parse(input: &[u8]) -> Result<Self, WireError> {
        if input.is_empty() {
            return Err(WireError::EmptyMessage);
        }
        if input.len() > MAX_MESSAGE_BYTES {
            return Err(WireError::MessageTooLarge {
                limit: MAX_MESSAGE_BYTES,
            });
        }
        if input.last() != Some(&b'\'') {
            return Err(WireError::MissingSegmentTerminator);
        }

        let segments = parse_segment_sequence(input)?;
        let message = Self { segments };
        message.validate(input.len())?;
        Ok(message)
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, WireError> {
        self.validate_envelope_and_numbers()?;

        let mut encoded = self.encode_unchecked()?;
        if encoded.len() > MAX_MESSAGE_BYTES {
            return Err(WireError::MessageTooLarge {
                limit: MAX_MESSAGE_BYTES,
            });
        }

        let length = format!("{:012}", encoded.len());
        if length.len() != 12 {
            return Err(WireError::MessageLengthOverflow);
        }
        let range = header_length_range(&encoded).ok_or(WireError::InvalidMessageEnvelope)?;
        encoded[range].copy_from_slice(length.as_bytes());
        Ok(encoded)
    }

    pub(crate) fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub(crate) fn payload_segments(&self) -> Result<Vec<Segment>, WireError> {
        if self.has_security_envelope() {
            let payload = self.segments[2]
                .element(1)
                .and_then(Element::single_binary)
                .ok_or(WireError::InvalidSecurityEnvelope)?;
            parse_segment_sequence(payload)
        } else {
            Ok(self.segments[1..self.segments.len() - 1].to_vec())
        }
    }

    fn validate(&self, input_len: usize) -> Result<(), WireError> {
        self.validate_envelope_and_numbers()?;

        let declared = self.segments[0]
            .element(1)
            .and_then(Element::single_text)
            .and_then(parse_decimal_usize)
            .ok_or(WireError::InvalidMessageEnvelope)?;
        if declared != input_len {
            return Err(WireError::InvalidMessageLength {
                declared,
                actual: input_len,
            });
        }

        let header_number = self.segments[0]
            .element(4)
            .and_then(Element::single_text)
            .ok_or(WireError::InvalidMessageEnvelope)?;
        let trailer_number = self.segments[self.segments.len() - 1]
            .element(1)
            .and_then(Element::single_text)
            .ok_or(WireError::InvalidMessageEnvelope)?;
        if header_number != trailer_number {
            return Err(WireError::MismatchedMessageNumber);
        }
        Ok(())
    }

    fn validate_envelope_and_numbers(&self) -> Result<(), WireError> {
        let first = self
            .segments
            .first()
            .ok_or(WireError::InvalidMessageEnvelope)?;
        let last = self
            .segments
            .last()
            .ok_or(WireError::InvalidMessageEnvelope)?;
        if first.header().map(|header| (header.code, header.version))
            != Some((b"HNHBK".as_slice(), 3))
            || last.header().map(|header| (header.code, header.version))
                != Some((b"HNHBS".as_slice(), 1))
        {
            return Err(WireError::InvalidMessageEnvelope);
        }

        if self.has_security_envelope() {
            if self.segments.len() != 4 {
                return Err(WireError::InvalidSecurityEnvelope);
            }
            let inner = self.payload_segments()?;
            if inner.len() > MAX_SEGMENTS - 2 {
                return Err(WireError::TooManySegments {
                    limit: MAX_SEGMENTS,
                });
            }
            validate_segment_numbers(&inner, 2)?;
            let expected_trailer =
                u16::try_from(inner.len() + 2).map_err(|_| WireError::TooManySegments {
                    limit: MAX_SEGMENTS,
                })?;
            let actual_trailer = last
                .header()
                .ok_or(WireError::InvalidMessageEnvelope)?
                .number;
            if actual_trailer != expected_trailer {
                return Err(WireError::NonSequentialSegmentNumber {
                    expected: expected_trailer,
                    actual: actual_trailer,
                });
            }
            return Ok(());
        }

        if self.segments.iter().any(|segment| {
            segment
                .header()
                .is_some_and(|header| matches!(header.code, b"HNVSK" | b"HNVSD"))
        }) {
            return Err(WireError::InvalidSecurityEnvelope);
        }
        validate_segment_numbers(&self.segments, 1)
    }

    fn has_security_envelope(&self) -> bool {
        self.segments
            .get(1)
            .and_then(Segment::header)
            .is_some_and(|header| {
                header.code == b"HNVSK" && header.number == 998 && header.version == 3
            })
            && self
                .segments
                .get(2)
                .and_then(Segment::header)
                .is_some_and(|header| {
                    header.code == b"HNVSD" && header.number == 999 && header.version == 1
                })
    }

    fn encode_unchecked(&self) -> Result<Vec<u8>, WireError> {
        encode_segment_sequence(&self.segments)
    }
}

pub(crate) fn encode_segments(segments: &[Segment]) -> Result<Vec<u8>, WireError> {
    encode_segment_sequence(segments)
}

fn validate_segment_numbers(segments: &[Segment], first_number: u16) -> Result<(), WireError> {
    for (index, segment) in segments.iter().enumerate() {
        let position = usize::from(first_number) + index;
        let header = segment
            .header()
            .ok_or(WireError::InvalidSegmentHeader { segment: position })?;
        let expected = u16::try_from(position).map_err(|_| WireError::TooManySegments {
            limit: MAX_SEGMENTS,
        })?;
        if header.number != expected {
            return Err(WireError::NonSequentialSegmentNumber {
                expected,
                actual: header.number,
            });
        }
    }
    Ok(())
}

fn encode_segment_sequence(segments: &[Segment]) -> Result<Vec<u8>, WireError> {
    let mut output = Vec::new();
    for (segment_index, segment) in segments.iter().enumerate() {
        if segment.elements.is_empty() {
            return Err(WireError::InvalidSegmentHeader {
                segment: segment_index + 1,
            });
        }
        for (element_index, element) in segment.elements.iter().enumerate() {
            if element_index > 0 {
                output.push(b'+');
            }
            element.encode_into(&mut output);
        }
        output.push(b'\'');
    }
    Ok(output)
}

impl Segment {
    pub(crate) fn new(elements: Vec<Element>) -> Self {
        Self { elements }
    }

    pub(crate) fn elements(&self) -> &[Element] {
        &self.elements
    }

    pub(crate) fn element(&self, index: usize) -> Option<&Element> {
        self.elements.get(index)
    }

    pub(crate) fn header(&self) -> Option<SegmentHeader<'_>> {
        let components = &self.elements.first()?.components;
        if !(3..=4).contains(&components.len()) {
            return None;
        }
        let code = components[0].text_bytes()?;
        if code.is_empty()
            || code.len() > 6
            || !code
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return None;
        }
        let number = parse_decimal_u16(components[1].text_bytes()?)?;
        let version = parse_decimal_u16(components[2].text_bytes()?)?;
        let reference = match components.get(3) {
            Some(value) => Some(parse_decimal_u16(value.text_bytes()?)?),
            None => None,
        };
        Some(SegmentHeader {
            code,
            number,
            version,
            reference,
        })
    }
}

impl Element {
    pub(crate) fn new(components: Vec<Value>) -> Self {
        Self { components }
    }

    pub(crate) fn text(value: &str) -> Result<Self, WireError> {
        Ok(Self {
            components: vec![Value::text(value)?],
        })
    }

    pub(crate) fn components(&self) -> &[Value] {
        &self.components
    }

    fn single_text(&self) -> Option<&[u8]> {
        if self.components.len() == 1 {
            self.components[0].text_bytes()
        } else {
            None
        }
    }

    fn single_binary(&self) -> Option<&[u8]> {
        if self.components.len() == 1 {
            self.components[0].as_binary()
        } else {
            None
        }
    }

    fn encode_into(&self, output: &mut Vec<u8>) {
        for (index, component) in self.components.iter().enumerate() {
            if index > 0 {
                output.push(b':');
            }
            component.encode_into(output);
        }
    }
}

impl Value {
    pub(crate) fn text(value: &str) -> Result<Self, WireError> {
        if !mem::is_str_latin1(value) {
            return Err(WireError::NonLatin1Text);
        }
        Ok(Self::Text(mem::encode_latin1_lossy(value).into_owned()))
    }

    pub(crate) fn binary(value: Vec<u8>) -> Self {
        Self::Binary(value)
    }

    pub(crate) fn as_text(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::Text(bytes) => Some(mem::decode_latin1(bytes)),
            Self::Binary(_) => None,
        }
    }

    pub(crate) fn as_binary(&self) -> Option<&[u8]> {
        match self {
            Self::Text(_) => None,
            Self::Binary(bytes) => Some(bytes),
        }
    }

    fn text_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Text(bytes) => Some(bytes),
            Self::Binary(_) => None,
        }
    }

    fn encode_into(&self, output: &mut Vec<u8>) {
        match self {
            Self::Text(bytes) => {
                for byte in bytes {
                    if SYNTAX_BYTES.contains(byte) {
                        output.push(b'?');
                    }
                    output.push(*byte);
                }
            }
            Self::Binary(bytes) => {
                output.push(b'@');
                output.extend_from_slice(bytes.len().to_string().as_bytes());
                output.push(b'@');
                output.extend_from_slice(bytes);
            }
        }
    }
}

pub(crate) struct SegmentHeader<'a> {
    pub(crate) code: &'a [u8],
    pub(crate) number: u16,
    pub(crate) version: u16,
    pub(crate) reference: Option<u16>,
}

fn parse_decimal_u16(input: &[u8]) -> Option<u16> {
    if input.is_empty() || !input.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(input).ok()?.parse().ok()
}

fn parse_decimal_usize(input: &[u8]) -> Option<usize> {
    if input.is_empty() || !input.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(input).ok()?.parse().ok()
}

fn header_length_range(encoded: &[u8]) -> Option<std::ops::Range<usize>> {
    let first_plus = encoded.iter().position(|byte| *byte == b'+')?;
    let start = first_plus + 1;
    let end = encoded[start..]
        .iter()
        .position(|byte| *byte == b'+')
        .map(|relative| start + relative)?;
    (end - start == 12).then_some(start..end)
}

#[cfg(test)]
mod tests;
